-- The version for the cover: the app's, read from ../package.json, and the month
-- of the build, so neither goes stale. Used by modbench-manual.tex.
local f = assert(io.open("../package.json"))
local json = f:read("a")
f:close()
local version = assert(json:match('"version"%s*:%s*"([^"]+)"'), "no version in package.json")
tex.sprint("\\def\\mbversion{" .. version .. "}\\def\\mbdate{" .. os.date("%B %Y") .. "}")
