-- Foxtrot Volta I — World Script
-- Defines interactions for the imported Foxtrot level.

local timer = 0
local welcome_shown = false

function on_update()
    local delta = dt()
    timer = timer + delta

    -- Welcome message after 2 seconds
    if not welcome_shown and timer > 2.0 then
        welcome_shown = true
        print_log("Bienvenue dans la Taverne Foxtrot!")
        show_text("Bienvenue dans la Taverne Foxtrot!", 4.0)
    end

    -- Periodic ambient log every 30s
    if timer > 30.0 then
        timer = 0
        print_log("Foxtrot: ambient tick")
    end
end

function on_trigger_enter()
    print_log("Foxtrot: trigger zone entered")
    show_text("Zone detectee!", 2.0)
end
