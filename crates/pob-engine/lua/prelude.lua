-- Runs before PoB loads. The host passes the directory of PoB's runtime Lua libraries.
local luaDir = ...

package.path = luaDir .. "/?.lua;" .. luaDir .. "/?/init.lua;" .. package.path

-- PoB reads a build link to open from arg[1] at startup.
arg = {}

-- Stdout carries the protocol, so PoB's console output goes to stderr.
io.output(io.stderr)
function print(...)
	local parts = {}
	for i = 1, select("#", ...) do
		parts[i] = tostring((select(i, ...)))
	end
	io.stderr:write(table.concat(parts, "\t"), "\n")
end

-- PoB's files start with a "#@" line that its own loader skips and LuaJIT's doesn't.
function loadfile(name)
	local f, err = io.open(name, "rb")
	if not f then
		return nil, err
	end
	local src = f:read("*a")
	f:close()
	if src:sub(1, 1) == "#" then
		src = "--" .. src
	end
	return loadstring(src, "@" .. name)
end
function dofile(name)
	return assert(loadfile(name))()
end

-- Headless PoB only uses lua-utf8 to put thousands separators into numbers, which are ASCII, and
-- in the edit box code. The string library does the same for ASCII.
package.preload["lua-utf8"] = function()
	return {
		find = string.find,
		gsub = string.gsub,
		len = string.len,
		match = string.match,
		reverse = string.reverse,
		sub = string.sub,
		next = function(s, i, dir)
			local j = (i or 0) + (dir or 1)
			if j < 1 or j > #s then
				return nil
			end
			return j
		end,
	}
end
