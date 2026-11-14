-- ═══════════════════════════════════════════════════════════════
-- GAIA AWARENESS — Kenshi Environmental Intelligence System
-- A KenshiLua mod by Gaia × Kevin (ecoPrimal)
-- 
-- What it does: Adds a real-time awareness overlay showing:
--   • Nearby threats with danger levels
--   • Character stats and injury summary  
--   • Faction relations for current area
--   • Time of day and weather conditions
--   • Nearby loot and items of interest
--
-- Requires: RE_Kenshi + KenshiLua (v0.3.10+)
-- ═══════════════════════════════════════════════════════════════

local MOD_NAME = "Gaia Awareness"
local VERSION = "0.1.0"
local UPDATE_INTERVAL = 2.0 -- seconds between scans
local SCAN_RADIUS = 500.0   -- game units
local DANGER_RADIUS = 300.0

-- State
local lastUpdate = 0
local awarenessData = {
    threats = {},
    friendlies = {},
    neutrals = {},
    loot = {},
    injuries = {},
    faction_standing = {},
    environment = {}
}

-- ─── Utility Functions ───────────────────────────────────────

local function distance3d(a, b)
    local dx = a.x - b.x
    local dy = a.y - b.y
    local dz = a.z - b.z
    return math.sqrt(dx*dx + dy*dy + dz*dz)
end

local function getDangerLevel(char)
    local stats = char:getCharStats()
    if not stats then return 0 end
    
    local attack = stats:getStat(0) or 0  -- melee attack
    local defence = stats:getStat(1) or 0 -- melee defence
    local str = stats:getStat(4) or 0     -- strength
    
    return (attack + defence + str) / 3.0
end

local function getHealthSummary(char)
    local med = char:getMedicalSystem()
    if not med then return "Unknown" end
    
    local hp = {}
    local limbNames = {"Head", "Chest", "Stomach", "R.Arm", "L.Arm", "R.Leg", "L.Leg"}
    local worstLimb = nil
    local worstHP = 100
    
    for i = 0, 6 do
        local limbHP = med:getLimbHP(i)
        if limbHP and limbHP < worstHP then
            worstHP = limbHP
            worstLimb = limbNames[i+1]
        end
    end
    
    if worstHP > 75 then return "Healthy"
    elseif worstHP > 50 then return worstLimb .. " damaged"
    elseif worstHP > 25 then return worstLimb .. " critical!"
    elseif worstHP > 0 then return worstLimb .. " near death!"
    else return worstLimb .. " DESTROYED"
    end
end

local function getRaceIcon(char)
    local race = char:getRace()
    if not race then return "[?]" end
    local name = race:getName()
    if name == "Human" or name == "Greenlander" then return "[H]"
    elseif name == "Scorchlander" then return "[S]"
    elseif name == "Shek" then return "[K]"
    elseif name == "Hive" or name:find("Hive") then return "[V]"
    elseif name == "Skeleton" then return "[X]"
    else return "[A]" -- animal/other
    end
end

-- ─── Core Scan Function ──────────────────────────────────────

local function scanEnvironment()
    local world = getGameWorld()
    if not world then return end
    
    local player = getPlayerInterface()
    if not player then return end
    
    local selectedChars = player:getSelectedCharacters()
    if not selectedChars or #selectedChars == 0 then return end
    
    local mainChar = selectedChars[1]
    if not mainChar then return end
    
    local myPos = mainChar:getPosition()
    if not myPos then return end
    
    local myFaction = mainChar:getFaction()
    
    -- Reset scan data
    awarenessData.threats = {}
    awarenessData.friendlies = {}
    awarenessData.neutrals = {}
    
    -- Scan nearby characters
    local chars = world:getCharactersInRange(myPos, SCAN_RADIUS)
    if chars then
        for i = 1, #chars do
            local char = chars[i]
            if char and char ~= mainChar then
                local cPos = char:getPosition()
                local dist = distance3d(myPos, cPos)
                local cFaction = char:getFaction()
                local danger = getDangerLevel(char)
                local isDead = char:isDead()
                
                local entry = {
                    name = char:getName() or "Unknown",
                    race = getRaceIcon(char),
                    distance = math.floor(dist),
                    danger = math.floor(danger),
                    health = getHealthSummary(char),
                    dead = isDead,
                    faction = cFaction and cFaction:getName() or "None"
                }
                
                if isDead then
                    -- Skip dead chars for threats
                elseif cFaction and myFaction then
                    local relations = cFaction:getRelationTo(myFaction)
                    if relations and relations < -50 then
                        table.insert(awarenessData.threats, entry)
                    elseif relations and relations > 25 then
                        table.insert(awarenessData.friendlies, entry)
                    else
                        table.insert(awarenessData.neutrals, entry)
                    end
                else
                    table.insert(awarenessData.neutrals, entry)
                end
            end
        end
    end
    
    -- Sort threats by distance (closest first)
    table.sort(awarenessData.threats, function(a, b) return a.distance < b.distance end)
    
    -- Get environment info
    local env = world:getEnvironmentManager()
    if env then
        awarenessData.environment = {
            timeOfDay = env:getTimeOfDay() or 0,
            dayPhase = env:isDayTime() and "Day" or "Night",
        }
    end
    
    -- Get own squad health summary
    awarenessData.injuries = {}
    local squads = player:getActiveSquads()
    if squads then
        for i = 1, #squads do
            local squad = squads[i]
            if squad then
                local members = squad:getMembers()
                if members then
                    for j = 1, #members do
                        local m = members[j]
                        if m then
                            local hp = getHealthSummary(m)
                            if hp ~= "Healthy" then
                                table.insert(awarenessData.injuries, {
                                    name = m:getName(),
                                    status = hp
                                })
                            end
                        end
                    end
                end
            end
        end
    end
end

-- ─── Display Functions ───────────────────────────────────────

local function formatThreatLine(entry)
    local dangerStr = ""
    if entry.danger > 70 then dangerStr = " !!DEADLY!!"
    elseif entry.danger > 50 then dangerStr = " !Dangerous"
    elseif entry.danger > 30 then dangerStr = " Moderate"
    else dangerStr = " Low"
    end
    
    return string.format("  %s %s [%dm] Lv%d%s (%s)",
        entry.race, entry.name, entry.distance, entry.danger, dangerStr, entry.faction)
end

local function printAwareness()
    print("╔══════════════════════════════════════╗")
    print("║       GAIA AWARENESS v" .. VERSION .. "        ║")
    print("╠══════════════════════════════════════╣")
    
    -- Environment
    local env = awarenessData.environment
    if env.timeOfDay then
        local hours = math.floor(env.timeOfDay)
        local mins = math.floor((env.timeOfDay - hours) * 60)
        print(string.format("║ Time: %02d:%02d (%s)", hours, mins, env.dayPhase or "?"))
    end
    
    -- Threats
    print("║")
    if #awarenessData.threats > 0 then
        print("║ ⚠ THREATS (" .. #awarenessData.threats .. "):")
        for _, t in ipairs(awarenessData.threats) do
            print("║" .. formatThreatLine(t))
        end
    else
        print("║ ✓ No threats detected")
    end
    
    -- Friendlies
    if #awarenessData.friendlies > 0 then
        print("║")
        print("║ ♦ Friendlies: " .. #awarenessData.friendlies)
    end
    
    -- Neutrals
    if #awarenessData.neutrals > 0 then
        print("║ ○ Neutrals: " .. #awarenessData.neutrals)
    end
    
    -- Squad injuries
    if #awarenessData.injuries > 0 then
        print("║")
        print("║ ✚ INJURIES:")
        for _, inj in ipairs(awarenessData.injuries) do
            print("║   " .. inj.name .. ": " .. inj.status)
        end
    end
    
    print("╚══════════════════════════════════════╝")
end

-- ─── Event Hooks ─────────────────────────────────────────────

-- Update on frame tick
Events.on("MyGUI::Gui::eventFrameStart", function(time)
    lastUpdate = lastUpdate + time
    if lastUpdate >= UPDATE_INTERVAL then
        lastUpdate = 0
        scanEnvironment()
    end
end)

-- Print awareness on key press (F9)
Events.on("InputHandler::keyDownEvent", function(self, keyCode)
    if keyCode == 0x43 then -- F9
        scanEnvironment()
        printAwareness()
    end
end)

-- Alert on combat start
Events.on("Character::_NV_hitByMeleeAttack", function(self, cutDir, damage, attacker, attack, comboID)
    local player = getPlayerInterface()
    if not player then return end
    
    local selected = player:getSelectedCharacters()
    if selected then
        for _, char in ipairs(selected) do
            if char == self then
                local atkName = attacker and attacker:getName() or "Unknown"
                local danger = getDangerLevel(attacker)
                print(string.format("[GAIA] %s attacked by %s (Lv%d)!", 
                    self:getName(), atkName, math.floor(danger)))
                break
            end
        end
    end
end)

-- Alert on character death
Events.on("Character::declareDead", function(self)
    local name = self:getName() or "Someone"
    local faction = self:getFaction()
    local fName = faction and faction:getName() or "Unknown"
    print(string.format("[GAIA] %s (%s) has died.", name, fName))
end)

-- Alert on faction encounter
Events.on("PlayerInterface::encounterFaction", function(self, faction)
    if faction then
        print(string.format("[GAIA] New faction encountered: %s", faction:getName() or "Unknown"))
    end
end)

-- Startup message
print("═══════════════════════════════════════")
print("  GAIA AWARENESS v" .. VERSION .. " loaded")
print("  Press F9 for awareness scan")
print("  Auto-scan every " .. UPDATE_INTERVAL .. "s")
print("  Scan radius: " .. SCAN_RADIUS .. " units")
print("  by ecoPrimal (gaia@primals.eco)")
print("═══════════════════════════════════════")
