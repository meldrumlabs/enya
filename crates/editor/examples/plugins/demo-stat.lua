-- Demo Stat Plugin
-- Demonstrates the custom stat and gauge pane features with auto-refresh
--
-- Usage:
-- 1. Copy this file to ~/.config/enya/plugins/
-- 2. Restart Enya
-- 3. Type :demo-stat to show a stat pane (auto-refreshes every 5 seconds)
-- 4. Type :demo-gauge to show a gauge pane (auto-refreshes every 5 seconds)
-- 5. Type :demo-update to manually update the values

plugin = {
    name = "demo-stat",
    version = "0.1.0",
    description = "Demo plugin showing custom stat and gauge panes with auto-refresh"
}

-- Generate random value with some variance
local function random_value(base, variance)
    return base + (math.random() * 2 - 1) * variance
end

-- Generate sparkline data (last 20 values)
local function generate_sparkline(base, variance)
    local data = {}
    for i = 1, 20 do
        table.insert(data, random_value(base, variance))
    end
    return data
end

-- Refresh the CPU stat data
local function refresh_cpu_stat()
    local cpu_value = random_value(45, 25)
    enya.set_stat_data("demo-cpu-stat", {
        value = cpu_value,
        sparkline = generate_sparkline(cpu_value, 10),
        change_value = random_value(0, 15),
        change_period = "vs last hour",
        thresholds = {
            { value = 0, color = "green" },
            { value = 50, color = "yellow" },
            { value = 80, color = "red" }
        }
    })
end

-- Refresh the memory gauge data
local function refresh_memory_gauge()
    local mem_value = random_value(62, 20)
    enya.set_gauge_data("demo-memory-gauge", {
        value = mem_value,
        thresholds = {
            { value = 0, color = "green" },
            { value = 60, color = "yellow" },
            { value = 85, color = "red" }
        }
    })
end

-- Register a custom stat pane type with auto-refresh
enya.register_stat_pane("demo-cpu-stat", {
    title = "CPU Usage",
    unit = "%",
    refresh_interval = 5  -- Auto-refresh every 5 seconds
}, refresh_cpu_stat)

-- Register a custom gauge pane type with auto-refresh
enya.register_gauge_pane("demo-memory-gauge", {
    title = "Memory Usage",
    unit = "%",
    min = 0,
    max = 100,
    refresh_interval = 5  -- Auto-refresh every 5 seconds
}, refresh_memory_gauge)

-- Command to show the demo stat pane
enya.register_command("demo-stat", {
    description = "Show demo CPU stat pane (auto-refreshes every 5s)"
}, function(args)
    -- Add the pane
    enya.add_stat_pane("demo-cpu-stat")

    -- Populate with initial data
    refresh_cpu_stat()

    enya.notify("info", "Demo stat pane added (auto-refreshes every 5s)")
    return true
end)

-- Command to show the demo gauge pane
enya.register_command("demo-gauge", {
    description = "Show demo memory gauge pane (auto-refreshes every 5s)"
}, function(args)
    -- Add the pane
    enya.add_gauge_pane("demo-memory-gauge")

    -- Populate with initial data
    refresh_memory_gauge()

    enya.notify("info", "Demo gauge pane added (auto-refreshes every 5s)")
    return true
end)

-- Command to manually update both stat and gauge
enya.register_command("demo-update", {
    description = "Manually update demo stat and gauge values"
}, function(args)
    refresh_cpu_stat()
    refresh_memory_gauge()
    enya.notify("info", "Demo values updated manually")
    return true
end)

-- Command to simulate an error
enya.register_command("demo-stat-error", {
    description = "Simulate a fetch error in demo stat"
}, function(args)
    enya.set_stat_data("demo-cpu-stat", {
        error = "Connection refused: unable to reach metrics server"
    })
    enya.notify("warn", "Simulated error shown in demo stat")
    return true
end)

-- Lifecycle hooks
function on_activate()
    enya.log("info", "Demo stat plugin activated! Type :demo-stat or :demo-gauge to try it.")
end

function on_deactivate()
    enya.log("info", "Demo stat plugin deactivated")
end
