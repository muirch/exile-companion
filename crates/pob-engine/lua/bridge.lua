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
		error("Path of Building couldn't open the build. " .. msg, 0)
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
		error("Path of Building couldn't open the build. " .. table.concat(lines, " "), 0)
	end
	return bridge.summary()
end

-- A share code, the base64 zlib text PoB's Import/Export tab and the build sites use.
function bridge.load_code(params)
	local xml = Inflate(common.base64.decode(params.code:gsub("-", "+"):gsub("_", "/")))
	if not xml then
		error("that isn't a Path of Building code", 0)
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

-- Reference builds, usually guides, loaded next to the player's build with PoB's own compare
-- entry. Advice for new players is the difference to one of these, not whatever PoB rates high.
local references = {}

local function refEntry(index)
	local ref = references[tonumber(index) or #references]
	if not ref then
		error("load a reference build first", 0)
	end
	return ref
end

-- Value of each notable and keystone: what the build loses without the ones it has (ascendancy
-- included), and what the ones it can reach within `max_points` would add, per point spent. Ranked
-- by DPS and EHP percent together, so a node that trades one for the other nets out. With
-- `reference`, only nodes that reference build takes count as reachable.
function bridge.node_power(params)
	local limit = params.limit or 10
	local only = params.reference and refEntry(params.reference)
	local maxPoints = params.max_points or (only and 1000 or 3)
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
			elseif not node.ascendancyName and node.path and #node.path <= maxPoints
				and (not only or only.spec.allocNodes[node.id]) then
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
		error("no socket group " .. tostring(index), 0)
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

-- What changes between two outputs, over the stats PoB shows in the sidebar and in its
-- "Equipping this will give you" tooltips.
local function statDiffs(before, after)
	local diffs, seen = {}, {}
	for _, statData in ipairs(build.displayStats) do
		local stat = statData.stat
		if stat and not statData.childStat and not seen[stat] then
			seen[stat] = true
			local a, b = num(before[stat]), num(after[stat])
			if a and b and math.abs(b - a) > 1e-9 * math.max(1, math.abs(a)) then
				local better = b > a
				if statData.lowerIsBetter then
					better = not better
				end
				diffs[#diffs + 1] = {
					stat = stat,
					label = statData.label,
					before = a,
					after = b,
					pct = a ~= 0 and (b - a) / math.abs(a) * 100 or nil,
					better = better,
				}
			end
		end
	end
	return diffs
end

local function measure(before, after)
	return {
		dps_pct = pct(dps(after), dps(before)),
		ehp_pct = pct(after.TotalEHP, before.TotalEHP),
		life = (num(after.Life) or 0) - (num(before.Life) or 0),
		changes = statDiffs(before, after),
	}
end

-- Mods with what PoB couldn't calculate marked, and every other property PoB kept from the item
-- text (quality, influence, league mechanics like Intangibility). New properties come through
-- this way without the bridge knowing them, and the model can look them up.
local function itemInfo(item)
	local mods, notCalculated, isMod = {}, {}, {}
	for _, list in ipairs({ item.enchantModLines, item.implicitModLines, item.explicitModLines }) do
		for _, m in ipairs(list or {}) do
			mods[#mods + 1] = m.line
			isMod[m.line] = true
			if m.extra then
				notCalculated[#notCalculated + 1] = m.line
			end
		end
	end
	local properties = {}
	for line in item:BuildRaw():gmatch("[^\r\n]+") do
		-- Mod lines carry {tags} in front, like {crafted} or {range:0.5}.
		local plain = line:gsub("^{[^}]*}", ""):gsub("^{[^}]*}", ""):gsub("^{[^}]*}", "")
		if not isMod[plain] and plain ~= item.title and plain ~= item.baseName and plain ~= item.name
			and not plain:match("^Rarity:") and not plain:match("^Implicits:") and not plain:match("^Unique ID:") then
			properties[#properties + 1] = plain
		end
	end
	return {
		name = item.name,
		base = item.baseName,
		rarity = item.rarity,
		mods = mods,
		not_calculated = notCalculated,
		properties = properties,
	}
end

-- Jewels that conquer or rewire passives need a rebuilt tree to compare. PoB does that with a
-- function local to ItemsTab, so these comparisons leave the tree as it is and say so.
local function changesPassiveTree(item)
	return not not (item and item.type == "Jewel" and item.jewelData
		and (item.jewelData.conqueredBy or item.jewelRadiusIndex
			and (item.jewelData.intuitiveLeapLike or item.jewelData.impossibleEscapeKeystone)))
end

-- An item as the game copies it with Ctrl+C, compared against every slot it fits, the way
-- PoB's item tooltip does it. Best slot first.
function bridge.compare_item(params)
	local item = new("Item", params.text)
	if not item.base then
		error("Path of Building couldn't read that item", 0)
	end
	item:NormaliseQuality()
	item:BuildModList()
	local itemsTab = build.itemsTab
	local calc = build.calcsTab:GetMiscCalculator()
	local base = output()
	local weaponSet = itemsTab.activeItemSet.useSecondWeaponSet and 2 or 1
	local slots = {}
	for slotName, slot in pairs(itemsTab.slots) do
		if itemsTab:IsItemValidForSlot(item, slotName) and not slot.inactive
			and (not slot.weaponSet or slot.weaponSet == weaponSet) then
			local current = itemsTab.items[slot.selItemId]
			local entry = measure(base, calc({ repSlotName = slotName, repItem = item }))
			entry.slot = slotName
			entry.replaces = current and current.name or nil
			slots[#slots + 1] = entry
		end
	end
	table.sort(slots, function(a, b)
		return a.dps_pct + a.ehp_pct > b.dps_pct + b.ehp_pct
	end)
	return {
		item = itemInfo(item),
		slots = slots,
		tree_not_rebuilt = changesPassiveTree(item) or nil,
	}
end

local function findNode(ref)
	local node = build.spec.nodes[tonumber(ref)]
	if node then
		return node
	end
	local found
	for _, n in pairs(build.spec.nodes) do
		if n.dn == ref and (n.type == "Notable" or n.type == "Keystone" or not found) then
			found = n
		end
	end
	return found or error("no passive node " .. tostring(ref), 0)
end

-- PoB names supports without the " Support" the game shows.
local function gemName(name)
	return (tostring(name):gsub(" Support$", ""))
end

local function findGroup(ref)
	local list = build.skillsTab.socketGroupList
	if ref == nil then
		return list[build.mainSocketGroup]
	end
	if list[tonumber(ref)] then
		return list[tonumber(ref)]
	end
	for _, g in ipairs(list) do
		if g.displayLabel == ref or g.label == ref then
			return g
		end
		for _, gem in ipairs(g.gemList) do
			if gem.nameSpec == gemName(ref) then
				return g
			end
		end
	end
	error("no socket group " .. tostring(ref), 0)
end

-- Tries changes together and measures them against the build as it is. Nothing is kept: every
-- change is undone before this returns, also when PoB fails on one.
-- Each change is one of:
--   { add_nodes = {ids or names} }   with the path to them
--   { remove_nodes = {ids or names} }
--   { item = "<item text>", slot = "Helmet" }
--   { gem = "Name", group = <index, label or gem name>, enabled = bool, level = n, quality = n }
--   { add_gem = "Name", group = ..., level = n, quality = n }
--   { config = "var", value = v }
--   { custom_mod = "line" }
--   { level = n }
function bridge.what_if(params)
	local base = output()
	local override, undo = {}, {}
	local configChanged = false

	local function apply(change)
		if change.add_nodes or change.remove_nodes then
			local key = change.add_nodes and "addNodes" or "removeNodes"
			override[key] = override[key] or {}
			for _, ref in ipairs(change.add_nodes or change.remove_nodes) do
				local node = findNode(ref)
				for _, n in ipairs(change.add_nodes and node.path or { node }) do
					override[key][n] = true
				end
				override[key][node] = true
			end
		elseif change.item then
			local item = new("Item", change.item)
			if not item.base then
				error("Path of Building couldn't read the item for " .. tostring(change.slot), 0)
			end
			item:NormaliseQuality()
			item:BuildModList()
			override.repSlotName = change.slot or error("an item change needs a slot", 0)
			override.repItem = item
		elseif change.gem then
			local group = findGroup(change.group or change.gem)
			for _, gem in ipairs(group.gemList) do
				if gem.nameSpec == gemName(change.gem) then
					local old = { enabled = gem.enabled, level = gem.level, quality = gem.quality }
					if change.enabled ~= nil then gem.enabled = change.enabled end
					if change.level then gem.level = change.level end
					if change.quality then gem.quality = change.quality end
					build.skillsTab:ProcessSocketGroup(group)
					undo[#undo + 1] = function()
						gem.enabled, gem.level, gem.quality = old.enabled, old.level, old.quality
						build.skillsTab:ProcessSocketGroup(group)
					end
					return
				end
			end
			error("no gem " .. change.gem .. " in that group", 0)
		elseif change.add_gem then
			local group = findGroup(change.group)
			local gem = { nameSpec = gemName(change.add_gem), level = change.level or 20, quality = change.quality or 0, enabled = true, enableGlobal1 = true, enableGlobal2 = true, count = 1 }
			table.insert(group.gemList, gem)
			build.skillsTab:ProcessSocketGroup(group)
			undo[#undo + 1] = function()
				for i, g in ipairs(group.gemList) do
					if g == gem then
						table.remove(group.gemList, i)
						break
					end
				end
				build.skillsTab:ProcessSocketGroup(group)
			end
			if not gem.gemData then
				error("Path of Building doesn't know the gem " .. change.add_gem, 0)
			end
		elseif change.config then
			local input = build.configTab.input
			local old = input[change.config]
			input[change.config] = change.value
			configChanged = true
			undo[#undo + 1] = function() input[change.config] = old end
		elseif change.custom_mod then
			local set = build.configTab.configSets[build.configTab.activeConfigSetId]
			local block = { title = "what if", enabled = true, text = change.custom_mod }
			table.insert(set.customModsList, block)
			configChanged = true
			undo[#undo + 1] = function()
				for i, b in ipairs(set.customModsList) do
					if b == block then
						table.remove(set.customModsList, i)
						break
					end
				end
			end
		elseif change.level then
			local old = build.characterLevel
			build.characterLevel = change.level
			undo[#undo + 1] = function() build.characterLevel = old end
		else
			error("unknown change", 0)
		end
	end

	local buildFlag = build.buildFlag
	local ok, result = pcall(function()
		for _, change in ipairs(params.changes or {}) do
			apply(change)
		end
		if configChanged then
			build.configTab:BuildModList()
		end
		local calc = build.calcsTab:GetMiscCalculator()
		return measure(base, calc(override))
	end)
	for i = #undo, 1, -1 do
		undo[i]()
	end
	if configChanged then
		build.configTab:BuildModList()
	end
	-- Building the mod lists flags a rebuild. The state is back as it was, so it isn't needed.
	build.buildFlag = buildFlag
	if not ok then
		error(result, 0)
	end
	return result
end

-- The changes what_if takes, made for good. Returns what they did and the build's new XML. When
-- one fails, the build is put back as it was.
function bridge.apply_changes(params)
	local before = build:SaveDB("code")
	local base = output()
	local configChanged = false

	local function apply(change)
		if change.add_nodes then
			for _, ref in ipairs(change.add_nodes) do
				local node = findNode(ref)
				if not node.alloc then
					if not node.path then
						error("the tree can't reach " .. node.dn, 0)
					end
					build.spec:AllocNode(node)
				end
			end
		elseif change.remove_nodes then
			-- Nodes only connected through this one go with it, like in PoB.
			for _, ref in ipairs(change.remove_nodes) do
				local node = findNode(ref)
				if node.alloc then
					build.spec:DeallocNode(node)
				end
			end
		elseif change.item then
			local items = build.itemsTab
			local slot = items.slots[change.slot or error("an item change needs a slot", 0)]
			if not slot then
				error("no item slot " .. change.slot, 0)
			end
			local item = new("Item", change.item)
			if not item.base then
				error("Path of Building couldn't read the item for " .. change.slot, 0)
			end
			item:NormaliseQuality()
			if not items:IsItemValidForSlot(item, change.slot) then
				error("that item doesn't fit " .. change.slot, 0)
			end
			items:AddItem(item, true)
			slot:SetSelItemId(item.id)
			items:PopulateSlots()
		elseif change.gem then
			local group = findGroup(change.group or change.gem)
			for _, gem in ipairs(group.gemList) do
				if gem.nameSpec == gemName(change.gem) then
					if change.enabled ~= nil then gem.enabled = change.enabled end
					if change.level then gem.level = change.level end
					if change.quality then gem.quality = change.quality end
					build.skillsTab:ProcessSocketGroup(group)
					return
				end
			end
			error("no gem " .. change.gem .. " in that group", 0)
		elseif change.add_gem then
			local group = findGroup(change.group)
			local gem = { nameSpec = gemName(change.add_gem), level = change.level or 20, quality = change.quality or 0, enabled = true, enableGlobal1 = true, enableGlobal2 = true, count = 1 }
			table.insert(group.gemList, gem)
			build.skillsTab:ProcessSocketGroup(group)
			if not gem.gemData then
				error("Path of Building doesn't know the gem " .. change.add_gem, 0)
			end
		elseif change.config then
			build.configTab.input[change.config] = change.value
			configChanged = true
		elseif change.custom_mod then
			local set = build.configTab.configSets[build.configTab.activeConfigSetId]
			table.insert(set.customModsList, { title = "Exile Companion", enabled = true, text = change.custom_mod })
			configChanged = true
		elseif change.level then
			build.characterLevel = change.level
			-- Otherwise PoB sets the level back from the points spent on the next frame.
			build.characterLevelAutoMode = false
		else
			error("unknown change", 0)
		end
	end

	local ok, err = pcall(function()
		for _, change in ipairs(params.changes or {}) do
			apply(change)
		end
		if configChanged then
			build.configTab:BuildModList()
		end
		rebuild()
	end)
	if not ok then
		loadBuildFromXML(before, build.buildName)
		error(err, 0)
	end
	local result = measure(base, output())
	result.xml = build:SaveDB("code")
	return result
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

local CLUSTER_NODE_OFFSET = 65536
local GEAR_SLOTS = {
	"Weapon 1", "Weapon 2", "Helmet", "Body Armour", "Gloves", "Boots", "Amulet",
	"Ring 1", "Ring 2", "Belt", "Flask 1", "Flask 2", "Flask 3", "Flask 4", "Flask 5",
}

local function setList(orderList, sets, activeId)
	local list = {}
	for i, id in ipairs(orderList or {}) do
		local set = sets[id]
		list[#list + 1] = { id = id, title = set and set.title or ("Set " .. i), active = id == activeId }
	end
	return list
end

local function referenceInfo(index)
	local ref = refEntry(index)
	local env = ref.calcsTab.mainEnv
	local skill = env and env.player and env.player.mainSkill
	local specs = {}
	for i, spec in ipairs(ref.treeTab.specList) do
		specs[#specs + 1] = { id = i, title = spec.title or ("Tree " .. i), active = i == ref.treeTab.activeSpec }
	end
	return {
		reference = index,
		label = ref.label,
		class = ref.spec.curClassName,
		ascendancy = ref.spec.curAscendClassName,
		level = ref.characterLevel,
		main_skill = skill and skill.activeEffect and skill.activeEffect.grantedEffect.name or nil,
		trees = specs,
		item_sets = setList(ref.itemsTab.itemSetOrderList, ref.itemsTab.itemSets, ref.itemsTab.activeItemSetId),
		skill_sets = setList(ref.skillsTab.skillSetOrderList, ref.skillsTab.skillSets, ref.skillsTab.activeSkillSetId),
	}
end

-- Loads a guide or any other build to compare the player's build with. Several can be loaded,
-- each gets an index.
function bridge.load_reference(params)
	local ref = new("CompareEntry", params.xml, params.label or "Guide")
	if not (ref and ref.calcsTab and ref.calcsTab.mainOutput) then
		error("Path of Building couldn't open the reference build", 0)
	end
	references[#references + 1] = ref
	-- The compare entry shares PoB's global caches with the player's build.
	rebuild()
	return referenceInfo(#references)
end

-- Picks the stage of a guide to compare with: its tree, item set and skill set, by id from
-- reference_info.
function bridge.set_reference_stage(params)
	local ref = refEntry(params.reference)
	if params.tree then ref:SetActiveSpec(tonumber(params.tree)) end
	if params.item_set then ref:SetActiveItemSet(tonumber(params.item_set)) end
	if params.skill_set then ref:SetActiveSkillSet(tonumber(params.skill_set)) end
	rebuild()
	return referenceInfo(params.reference or #references)
end

function bridge.reference_info(params)
	return referenceInfo(params.reference or #references)
end

local function gemNames(group)
	local names = {}
	for _, gem in ipairs(group and group.gemList or {}) do
		if gem.enabled ~= false then
			names[gem.nameSpec] = gem
		end
	end
	return names
end

local function activeGemOf(group)
	for _, gem in ipairs(group and group.gemList or {}) do
		local support = gem.gemData and gem.gemData.grantedEffect and gem.gemData.grantedEffect.support
		if not support and gem.enabled ~= false then
			return gem.nameSpec
		end
	end
end

local function sortByValue(list, best)
	table.sort(list, function(a, b)
		local sa, sb = a.dps_pct + a.ehp_pct, b.dps_pct + b.ehp_pct
		if best then return sa > sb end
		return sa < sb
	end)
end

local function cut(list, limit)
	for i = #list, limit + 1, -1 do
		list[i] = nil
	end
	return list
end

-- The player's build against a reference build: what the reference has that the player doesn't,
-- and the other way round, each measured on the player's build. Passives with the path to them,
-- gear slot by slot, supports of the main skill, other skill groups, and Configuration
-- assumptions that differ.
function bridge.compare_reference(params)
	local ref = refEntry(params.reference)
	local limit = params.limit or 12
	local base = output()
	local calc = build.calcsTab:GetMiscCalculator()
	local function value(override)
		local r = measure(base, calc(override))
		r.changes = nil
		return r
	end

	local missing, extra, smallDiff = {}, {}, 0
	for id, _ in pairs(ref.spec.allocNodes) do
		if type(id) == "number" and id < CLUSTER_NODE_OFFSET and not build.spec.allocNodes[id] then
			local node = build.spec.nodes[id]
			if node and not node.ascendancyName and (node.type == "Notable" or node.type == "Keystone") then
				local add = { [node] = true }
				for _, n in ipairs(node.path or {}) do
					add[n] = true
				end
				local r = value({ addNodes = add })
				r.node = nodeInfo(node)
				r.points = math.max(1, #(node.path or {}))
				missing[#missing + 1] = r
			elseif node and node.type == "Normal" and not node.ascendancyName then
				smallDiff = smallDiff + 1
			end
		end
	end
	for id, node in pairs(build.spec.allocNodes) do
		if type(id) == "number" and id < CLUSTER_NODE_OFFSET and not ref.spec.allocNodes[id]
			and not node.ascendancyName and (node.type == "Notable" or node.type == "Keystone") then
			local r = value({ removeNodes = { [node] = true } })
			r.node = nodeInfo(node)
			extra[#extra + 1] = r
		end
	end
	sortByValue(missing, true)
	sortByValue(extra, false)

	local ascendancy = { missing = {}, extra = {} }
	for id, node in pairs(ref.spec.allocNodes) do
		if node.ascendancyName and node.type == "Notable" and not build.spec.allocNodes[id] then
			ascendancy.missing[#ascendancy.missing + 1] = node.dn
		end
	end
	for id, node in pairs(build.spec.allocNodes) do
		if node.ascendancyName and node.type == "Notable" and not ref.spec.allocNodes[id] then
			ascendancy.extra[#ascendancy.extra + 1] = node.dn
		end
	end

	local gear = {}
	for _, slotName in ipairs(GEAR_SLOTS) do
		local rSlot = ref.itemsTab.slots[slotName]
		local rItem = rSlot and ref.itemsTab.items[rSlot.selItemId]
		local pSlot = build.itemsTab.slots[slotName]
		local pItem = pSlot and build.itemsTab.items[pSlot.selItemId]
		if rItem and rItem.raw and not (pItem and pItem.name == rItem.name) then
			local copy = new("Item", rItem.raw)
			copy:NormaliseQuality()
			copy:BuildModList()
			local r = value({ repSlotName = slotName, repItem = copy })
			r.slot = slotName
			r.reference_item = itemInfo(copy)
			r.player_item = pItem and pItem.name or nil
			gear[#gear + 1] = r
		end
	end
	sortByValue(gear, true)

	-- The reference group with the player's main skill in it, or the reference's own main group.
	local pGroup = build.skillsTab.socketGroupList[build.mainSocketGroup]
	local mainSkill = activeGemOf(pGroup)
	local rGroup
	for _, g in ipairs(ref.skillsTab.socketGroupList) do
		if mainSkill and gemNames(g)[mainSkill] then
			rGroup = g
		end
	end
	rGroup = rGroup or ref.skillsTab.socketGroupList[ref.mainSocketGroup]
	local pGems, rGems = gemNames(pGroup), gemNames(rGroup)
	local supports = { missing = {}, extra = {} }
	for name, gem in pairs(rGems) do
		if not pGems[name] and name ~= activeGemOf(rGroup) then
			local r = bridge.what_if({ changes = { { add_gem = name, group = build.mainSocketGroup, level = gem.level, quality = gem.quality } } })
			r.changes = nil
			r.gem = name
			supports.missing[#supports.missing + 1] = r
		end
	end
	for name, _ in pairs(pGems) do
		if not rGems[name] and name ~= mainSkill then
			local r = bridge.what_if({ changes = { { gem = name, group = build.mainSocketGroup, enabled = false } } })
			r.changes = nil
			r.gem = name
			supports.extra[#supports.extra + 1] = r
		end
	end
	sortByValue(supports.missing, true)
	sortByValue(supports.extra, false)

	-- Other skills the reference uses (auras, curses, guards, movement) that the player has nowhere.
	local playerGems = {}
	for _, g in ipairs(build.skillsTab.socketGroupList) do
		for name in pairs(gemNames(g)) do
			playerGems[name] = true
		end
	end
	local otherSkills = {}
	for _, g in ipairs(ref.skillsTab.socketGroupList) do
		if g ~= rGroup and g.enabled ~= false then
			local active = activeGemOf(g)
			if active and not playerGems[active] then
				local list = {}
				for name in pairs(gemNames(g)) do
					list[#list + 1] = name
				end
				table.sort(list)
				otherSkills[#otherSkills + 1] = { skill = active, slot = g.slot, gems = list }
			end
		end
	end

	local config = {}
	configOptions = configOptions or LoadModule("Modules/ConfigOptions")
	local pIn, rIn = build.configTab.input, ref.configTab.input
	for _, v in ipairs(configOptions) do
		if v.var and pIn[v.var] ~= rIn[v.var] and (pIn[v.var] ~= nil or rIn[v.var] ~= nil) then
			config[#config + 1] = {
				var = v.var,
				label = (strip(v.label or v.var):gsub(":$", "")),
				player = pIn[v.var],
				reference = rIn[v.var],
			}
		end
	end

	return {
		reference = referenceInfo(params.reference or #references),
		player = { level = build.characterLevel, main_skill = mainSkillName() },
		-- Every stat PoB shows, player first, reference second.
		stats = statDiffs(base, ref.calcsTab.mainOutput),
		passives = { missing = cut(missing, limit), extra = cut(extra, limit), small_passives_missing = smallDiff },
		ascendancy = ascendancy,
		gear = cut(gear, limit),
		supports = supports,
		other_skills = otherSkills,
		config = config,
	}
end

local function specSummary(spec)
	local points, notables, ascendancy = 0, {}, {}
	for id, node in pairs(spec.allocNodes) do
		if type(id) == "number" and id < CLUSTER_NODE_OFFSET then
			if node.ascendancyName then
				if node.type == "Notable" then
					ascendancy[#ascendancy + 1] = node.dn
				end
			elseif node.type ~= "ClassStart" and node.type ~= "AscendClassStart" then
				points = points + 1
				if node.type == "Notable" or node.type == "Keystone" then
					notables[node.dn] = true
				end
			end
		end
	end
	table.sort(ascendancy)
	return points, notables, ascendancy
end

local function setGear(ref, set)
	local gear = {}
	for _, slotName in ipairs(GEAR_SLOTS) do
		local id = set and set[slotName] and set[slotName].selItemId
		local item = id and id > 0 and ref.itemsTab.items[id]
		if item then
			gear[slotName] = item.name
		end
	end
	return gear
end

local function setSkills(groups)
	local skills = {}
	for _, g in ipairs(groups or {}) do
		local active = activeGemOf(g)
		if active and g.enabled ~= false then
			local gems = {}
			for name in pairs(gemNames(g)) do
				if name ~= active then
					gems[#gems + 1] = name
				end
			end
			table.sort(gems)
			skills[#skills + 1] = { skill = active, slot = g.slot, supports = gems }
		end
	end
	return skills
end

local function skillSetGroups(ref, id)
	local set = ref.skillsTab.skillSets[id]
	if id == ref.skillsTab.activeSkillSetId then
		return ref.skillsTab.socketGroupList
	end
	return set and set.socketGroupList or {}
end

-- A guide's stages as it saved them: each passive tree with its points and the ascendancy
-- notables it adds, each item set's gear and each skill set's skills. Trees come in the guide's
-- order, which is usually the order to play them in.
function bridge.guide_stages(params)
	local ref = refEntry(params.reference)
	local trees, order, seen = {}, {}, {}
	for i, spec in ipairs(ref.treeTab.specList) do
		local points, _, asc = specSummary(spec)
		local new = {}
		for _, a in ipairs(asc) do
			if not seen[a] then
				seen[a] = true
				new[#new + 1] = a
			end
		end
		-- One group per tree that adds any. Within a tree the guide doesn't say which came first.
		if #new > 0 then
			order[#order + 1] = new
		end
		trees[#trees + 1] = { id = i, title = spec.title or ("Tree " .. i), points = points, ascendancy_added = new }
	end
	local itemSets = {}
	for _, id in ipairs(ref.itemsTab.itemSetOrderList) do
		local set = ref.itemsTab.itemSets[id]
		itemSets[#itemSets + 1] = { id = id, title = set and set.title or ("Set " .. id), gear = setGear(ref, set) }
	end
	local skillSets = {}
	for _, id in ipairs(ref.skillsTab.skillSetOrderList) do
		local set = ref.skillsTab.skillSets[id]
		skillSets[#skillSets + 1] = { id = id, title = set and set.title or ("Set " .. id), skills = setSkills(skillSetGroups(ref, id)) }
	end
	return {
		reference = params.reference or #references,
		level = ref.characterLevel,
		trees = trees,
		ascendancy_order = order,
		item_sets = itemSets,
		skill_sets = skillSets,
	}
end

-- What changes from one stage of a guide to another (`from` and `to` are {tree, item_set,
-- skill_set} ids from guide_stages): passives, ascendancy, gear, skills, and the stats PoB shows
-- for each stage. The player's build isn't involved.
function bridge.guide_stage_diff(params)
	local ref = refEntry(params.reference)
	local from, to = params.from or {}, params.to or {}
	local specs = ref.treeTab.specList
	local fromSpec = specs[tonumber(from.tree) or ref.treeTab.activeSpec] or error("no tree " .. tostring(from.tree), 0)
	local toSpec = specs[tonumber(to.tree) or ref.treeTab.activeSpec] or error("no tree " .. tostring(to.tree), 0)
	local fromPoints, fromNotables, fromAsc = specSummary(fromSpec)
	local toPoints, toNotables, toAsc = specSummary(toSpec)
	local added, removed = {}, {}
	for name in pairs(toNotables) do
		if not fromNotables[name] then added[#added + 1] = name end
	end
	for name in pairs(fromNotables) do
		if not toNotables[name] then removed[#removed + 1] = name end
	end
	table.sort(added)
	table.sort(removed)
	local hadAsc = {}
	for _, a in ipairs(fromAsc) do hadAsc[a] = true end
	local newAsc = {}
	for _, a in ipairs(toAsc) do
		if not hadAsc[a] then newAsc[#newAsc + 1] = a end
	end

	local fromGear = setGear(ref, ref.itemsTab.itemSets[tonumber(from.item_set) or ref.itemsTab.activeItemSetId])
	local toGear = setGear(ref, ref.itemsTab.itemSets[tonumber(to.item_set) or ref.itemsTab.activeItemSetId])
	local gear = {}
	for _, slotName in ipairs(GEAR_SLOTS) do
		if fromGear[slotName] ~= toGear[slotName] then
			gear[#gear + 1] = { slot = slotName, from = fromGear[slotName], to = toGear[slotName] }
		end
	end

	local fromSkills = setSkills(skillSetGroups(ref, tonumber(from.skill_set) or ref.skillsTab.activeSkillSetId))
	local toSkills = setSkills(skillSetGroups(ref, tonumber(to.skill_set) or ref.skillsTab.activeSkillSetId))
	local before = {}
	for _, sk in ipairs(fromSkills) do before[sk.skill] = sk end
	local skills = {}
	for _, sk in ipairs(toSkills) do
		local old = before[sk.skill]
		if not old then
			skills[#skills + 1] = { skill = sk.skill, change = "new", supports = sk.supports }
		elseif table.concat(old.supports, ",") ~= table.concat(sk.supports, ",") then
			skills[#skills + 1] = { skill = sk.skill, change = "supports", from = old.supports, to = sk.supports }
		end
		before[sk.skill] = nil
	end
	for name in pairs(before) do
		skills[#skills + 1] = { skill = name, change = "dropped" }
	end

	-- Stats need each stage calculated. The guide's own choice is put back afterwards.
	local saved = { tree = ref.treeTab.activeSpec, item_set = ref.itemsTab.activeItemSetId, skill_set = ref.skillsTab.activeSkillSetId }
	local function outputAt(stage)
		ref:SetActiveSpec(tonumber(stage.tree) or saved.tree)
		ref:SetActiveItemSet(tonumber(stage.item_set) or saved.item_set)
		ref:SetActiveSkillSet(tonumber(stage.skill_set) or saved.skill_set)
		return ref.calcsTab.mainOutput
	end
	local ok, result = pcall(function()
		local a = outputAt(from)
		local b = outputAt(to)
		return statDiffs(a, b)
	end)
	ref:SetActiveSpec(saved.tree)
	ref:SetActiveItemSet(saved.item_set)
	ref:SetActiveSkillSet(saved.skill_set)
	rebuild()
	if not ok then
		error(result, 0)
	end

	return {
		points = { from = fromPoints, to = toPoints },
		notables_added = added,
		notables_removed = removed,
		ascendancy_added = newAsc,
		gear = gear,
		skills = skills,
		stats = result,
	}
end

return bridge
