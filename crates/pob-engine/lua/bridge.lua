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

-- PoB's colour codes: ^0 to ^9 and ^xRRGGBB.
local function strip(text)
	return (tostring(text):gsub("%^x%x%x%x%x%x%x", ""):gsub("%^%d", ""))
end

-- The rebuild from Build:OnFrame, without drawing a frame of the open tab.
local function rebuild()
	wipeGlobalCache()
	build.outputRevision = build.outputRevision + 1
	build.buildFlag = false
	build.skillsTab:UpdateSocketGroups()
	build.calcsTab:BuildOutput()
	build:RefreshStatList()
	build.configTab.calcFunc, build.configTab.calcBase = build.calcsTab:GetMiscCalculator(build)
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

-- Without a manifest PoB runs in dev mode and saves the old build to Builds/~~temp~~.xml when
-- another one opens. We never want PoB to write builds on its own.
main.disableDevAutoSave = true

-- A build PoB can't open leaves a prompt or a popup behind instead of raising an error.
function bridge.load_build(params)
	main.popups = {}
	launch.promptMsg = nil
	loadBuildFromXML(params.xml, params.name or "build")
	if launch.promptMsg then
		local msg = strip(launch.promptMsg):match("Error:%s*(.-)\n") or strip(launch.promptMsg)
		launch.promptMsg = nil
		error("Path of Building couldn't open the build. " .. msg)
	end
	local popup = main.popups[1]
	if popup or not build.targetVersion then
		local lines = {}
		for _, c in pairs(popup and popup.controls or {}) do
			if type(c.label) == "string" and c.label ~= "" then
				lines[#lines + 1] = strip(c.label)
			end
		end
		main.popups = {}
		error("Path of Building couldn't open the build. " .. table.concat(lines, " "))
	end
	return bridge.summary()
end

-- A share code, the base64 zlib text PoB's Import/Export tab and the build sites use.
function bridge.load_code(params)
	local xml = Inflate(common.base64.decode(params.code:gsub("-", "+"):gsub("_", "/")))
	if not xml then
		error("that isn't a Path of Building code")
	end
	return bridge.load_build({ xml = xml, name = params.name })
end

-- The left sidebar as PoB shows it: the stat rows in its order and format, and its warnings.
function bridge.sidebar_stats()
	local rows = {}
	for _, row in ipairs(build.controls.statBox.list) do
		if row[1] then
			rows[#rows + 1] = { label = (strip(row[1]):gsub(":$", "")), value = row[2] and strip(row[2]) or nil }
		end
	end
	local warnings = {}
	for _, w in ipairs(build.controls.warnings.lines or {}) do
		warnings[#warnings + 1] = strip(w)
	end
	return { main_skill = mainSkillName(), stats = rows, warnings = warnings }
end

local configOptions

-- Configuration options that differ from PoB's defaults, and the custom modifier lines. Every
-- number depends on these (boss, buffs, charges, made-up mods), so answers must mention them.
local function configChanges()
	configOptions = configOptions or LoadModule("Modules/ConfigOptions")
	local input = build.configTab.input
	local changed = {}
	for _, v in ipairs(configOptions) do
		local value = v.var and input[v.var]
		if value ~= nil then
			local list = type(v.list) == "table" and v.list or nil
			local default = v.defaultState
			if v.type == "list" and list then
				local d = list[v.defaultIndex or 1]
				default = d and d.val
			elseif v.type == "check" then
				default = v.defaultState or false
			end
			if value ~= default then
				local shown = value
				for _, option in ipairs(list or {}) do
					if option.val == value then
						shown = strip(option.label)
					end
				end
				changed[#changed + 1] = { var = v.var, label = (strip(v.label or v.var):gsub(":$", "")), value = shown }
			end
		end
	end
	local custom = {}
	local set = build.configTab.configSets[build.configTab.activeConfigSetId]
	for _, block in ipairs(set and set.customModsList or {}) do
		if block.enabled ~= false then
			for line in (block.text or ""):gmatch("[^\r\n]+") do
				-- PoB skips lines that start with #, players use them to switch a mod off.
				if not line:match("^%s*#") then
					custom[#custom + 1] = line
				end
			end
		end
	end
	return changed, custom
end

-- The build as PoB saves it, and the share code PoB's Import/Export tab gives.
function bridge.export_build()
	local xml = build:SaveDB("code")
	local code = common.base64.encode(Deflate(xml)):gsub("+", "-"):gsub("/", "_")
	return { xml = xml, code = code }
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
	local calc = build.calcsTab:GetMiscCalculator()
	local gems = {}
	for _, gem in ipairs(group.gemList) do
		local entry = {
			name = gem.nameSpec,
			level = gem.level,
			quality = gem.quality,
			support = gem.gemData and gem.gemData.grantedEffect and gem.gemData.grantedEffect.support or false,
			enabled = gem.enabled ~= false,
		}
		-- The same toggle and calculate PoB's gem tooltip does.
		if entry.enabled then
			gem.enabled = false
			entry.dps_pct = pct(dps(calc()), base)
			gem.enabled = true
		end
		gems[#gems + 1] = entry
	end
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

	local config, customMods = configChanges()
	return {
		summary = bridge.summary(),
		config = config,
		custom_mods = customMods,
		items = items,
		socket_groups = groups,
		keystones = keystones,
		notables = notables,
		ascendancy = ascendancy,
	}
end

return bridge
