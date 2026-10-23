-- Request handlers for the engine host. Each takes a params table and returns a plain table the
-- host turns into JSON. Runs after headless.lua, so the globals `build` and `loadBuildFromXML`
-- exist. Everything here reads PoB internals (calcsTab, skillsTab, spec), which change between
-- PoB versions without notice.

local bridge = {}

local DAMAGE_TYPES = { "Physical", "Fire", "Cold", "Lightning", "Chaos" }
local AILMENTS = { "Stun", "Freeze", "Chill", "Shock", "Ignite", "Bleed", "Poison", "Curse" }

-- JSON has no NaN or infinity, and PoB puts both in its output.
local function num(x)
	if type(x) ~= "number" or x ~= x or x == math.huge or x == -math.huge then
		return nil
	end
	return x
end

local function pick(out, keys)
	local t = {}
	for _, k in ipairs(keys) do
		t[k] = num(out[k])
	end
	return t
end

local function pct(new, base)
	new, base = num(new), num(base)
	if not new or not base or base == 0 then
		return 0
	end
	return (new - base) / base * 100
end

local function output()
	return build.calcsTab.mainOutput
end

-- FullDPS sums the skills marked "include in full DPS". A build that marks none has only the main
-- skill's CombinedDPS.
local function dps(out)
	local full = num(out.FullDPS)
	if full and full > 0 then
		return full
	end
	return num(out.CombinedDPS)
end

local function mainSkillName()
	local env = build.calcsTab.mainEnv
	local skill = env and env.player and env.player.mainSkill
	return skill and skill.activeEffect and skill.activeEffect.grantedEffect.name or nil
end

local OFFENCE = {
	"FullDPS", "FullDotDPS", "CombinedDPS", "TotalDPS", "TotalDotDPS", "AverageDamage",
	"Speed", "CritChance", "CritMultiplier", "HitChance",
}
local DEFENCE = {
	"Life", "LifeUnreserved", "EnergyShield", "Mana", "ManaUnreserved", "Ward", "TotalEHP",
	"Armour", "Evasion", "EvadeChance", "BlockChance", "SpellBlockChance", "SpellSuppressionChance",
	"SpellSuppressionEffect", "LifeRegenRecovery", "LifeLeechGainRate", "EnergyShieldRegenRecovery",
	"EffectiveMovementSpeedMod",
}

local function skillDps(out)
	local list = {}
	for _, s in ipairs(out.SkillDPS or {}) do
		list[#list + 1] = { name = s.name, dps = num(s.dps), count = s.count }
	end
	return list
end

function bridge.summary()
	local out = output()
	return {
		class = build.spec.curClassName,
		ascendancy = build.spec.curAscendClassName,
		level = build.characterLevel,
		main_skill = mainSkillName(),
		offence = pick(out, OFFENCE),
		defence = pick(out, DEFENCE),
		skills = skillDps(out),
	}
end

function bridge.load_build(params)
	loadBuildFromXML(params.xml, params.name or "build")
	return bridge.summary()
end

function bridge.defences()
	local out = output()
	local types = {}
	for _, t in ipairs(DAMAGE_TYPES) do
		types[#types + 1] = {
			type = t,
			resist = num(out[t .. "Resist"]),
			resist_over_cap = num(out[t .. "ResistOverCap"]),
			max_hit = num(out[t .. "MaximumHitTaken"]),
			dot_ehp = num(out[t .. "DotEHP"]),
			avoid_chance = num(out["Avoid" .. t .. "DamageChance"]),
		}
	end
	local ailments = {}
	for _, a in ipairs(AILMENTS) do
		ailments[#ailments + 1] = { ailment = a, avoid_chance = num(out[a .. "AvoidChance"]) }
	end
	return {
		pools = pick(out, DEFENCE),
		damage_types = types,
		ailments = ailments,
		survival_time = num(out.EHPSurvivalTime),
	}
end

local function nodeInfo(node)
	return {
		id = node.id,
		name = node.dn,
		kind = node.type,
		ascendancy = node.ascendancyName,
		stats = node.sd,
	}
end

-- Value of each notable and keystone: what the build loses without the ones it has (ascendancy
-- included), and what the ones it can reach within `max_points` would add, per point spent. Ranked
-- by DPS and EHP percent together, so a node that trades one for the other nets out.
function bridge.node_power(params)
	local limit = params.limit or 10
	local maxPoints = params.max_points or 3
	local base = output()
	local calc = build.calcsTab:GetMiscCalculator()

	local function measure(override)
		local o = calc(override)
		return {
			dps_pct = pct(dps(o), dps(base)),
			life = (num(o.Life) or 0) - (num(base.Life) or 0),
			ehp_pct = pct(o.TotalEHP, base.TotalEHP),
		}
	end

	local taken, reachable = {}, {}
	for _, node in pairs(build.spec.nodes) do
		if node.type == "Notable" or node.type == "Keystone" then
			if node.alloc then
				local r = measure({ removeNodes = { [node] = true } })
				r.node = nodeInfo(node)
				taken[#taken + 1] = r
			elseif not node.ascendancyName and node.path and #node.path <= maxPoints then
				local add = {}
				for _, n in ipairs(node.path) do
					add[n] = true
				end
				local r = measure({ addNodes = add })
				r.node = nodeInfo(node)
				r.points = #node.path
				reachable[#reachable + 1] = r
			end
		end
	end

	local function score(r)
		return r.dps_pct + r.ehp_pct
	end
	table.sort(taken, function(a, b) return score(a) < score(b) end)
	table.sort(reachable, function(a, b) return score(a) / a.points > score(b) / b.points end)
	for i = #taken, limit + 1, -1 do taken[i] = nil end
	for i = #reachable, limit + 1, -1 do reachable[i] = nil end
	return { taken = taken, reachable = reachable }
end

-- DPS lost when each gem of a socket group is turned off, the main group by default.
function bridge.gem_value(params)
	local index = params.group or build.mainSocketGroup
	local group = build.skillsTab.socketGroupList[index]
	if not group then
		error("no socket group " .. tostring(index))
	end
	local base = dps(output())
	local gems = {}
	for _, gem in ipairs(group.gemList) do
		local entry = {
			name = gem.nameSpec,
			level = gem.level,
			quality = gem.quality,
			support = gem.gemData and gem.gemData.grantedEffect and gem.gemData.grantedEffect.support or false,
			enabled = gem.enabled ~= false,
		}
		if entry.enabled then
			gem.enabled = false
			build.buildFlag = true
			runCallback("OnFrame")
			entry.dps_pct = pct(dps(output()), base)
			gem.enabled = true
		end
		gems[#gems + 1] = entry
	end
	build.buildFlag = true
	runCallback("OnFrame")
	return { group = index, label = group.displayLabel or group.label, slot = group.slot, gems = gems }
end

-- Everything a reader needs to explain the build: gear with its mod lines, gem groups, and the
-- keystones and notables taken.
function bridge.build_info()
	local items = {}
	for _, slot in ipairs(build.itemsTab.orderedSlots) do
		local item = build.itemsTab.items[slot.selItemId]
		if item and not slot.inactive then
			local mods = {}
			for _, list in ipairs({ item.enchantModLines, item.implicitModLines, item.explicitModLines }) do
				for _, m in ipairs(list or {}) do
					mods[#mods + 1] = m.line
				end
			end
			items[#items + 1] = {
				slot = slot.slotName,
				name = item.name,
				base = item.baseName,
				rarity = item.rarity,
				mods = mods,
			}
		end
	end

	local groups = {}
	for i, group in ipairs(build.skillsTab.socketGroupList) do
		local gems = {}
		for _, gem in ipairs(group.gemList) do
			gems[#gems + 1] = { name = gem.nameSpec, level = gem.level, quality = gem.quality, enabled = gem.enabled ~= false }
		end
		groups[#groups + 1] = {
			index = i,
			label = group.displayLabel or group.label,
			slot = group.slot,
			enabled = group.enabled ~= false,
			main = i == build.mainSocketGroup,
			gems = gems,
		}
	end

	local keystones, notables, ascendancy = {}, {}, {}
	for _, node in pairs(build.spec.allocNodes) do
		if node.ascendancyName then
			if node.type == "Notable" then
				ascendancy[#ascendancy + 1] = nodeInfo(node)
			end
		elseif node.type == "Keystone" then
			keystones[#keystones + 1] = nodeInfo(node)
		elseif node.type == "Notable" then
			notables[#notables + 1] = nodeInfo(node)
		end
	end
	local byName = function(a, b) return a.name < b.name end
	table.sort(keystones, byName)
	table.sort(notables, byName)
	table.sort(ascendancy, byName)

	return {
		summary = bridge.summary(),
		items = items,
		socket_groups = groups,
		keystones = keystones,
		notables = notables,
		ascendancy = ascendancy,
	}
end

return bridge
