-- Demo Chart Plugin
-- Demonstrates the custom chart pane feature
--
-- Usage:
-- 1. Copy this file to ~/.enya/plugins/
-- 2. Restart Enya
-- 3. Type :demo-chart to show the demo chart
-- 4. Type :demo-chart-update to refresh with new data

plugin = {
    name = "demo-chart",
    version = "0.1.0",
    description = "Demo plugin showing custom chart panes"
}

-- Register a custom chart pane type during plugin load
enya.register_chart_pane("demo-metrics", {
    title = "Demo Metrics",
    y_unit = "ms"  -- Unit for Y-axis labels
})

-- Generate sample time series data
local function generate_demo_data()
    local now = os.time()
    local series = {}

    -- Series 1: API Latency (fluctuating around 50ms)
    local api_points = {}
    for i = 60, 1, -1 do
        local timestamp = now - (i * 60)  -- One point per minute, going back 60 minutes
        local value = 50 + math.random(-20, 30) + math.sin(i / 10) * 10
        table.insert(api_points, { timestamp = timestamp, value = value })
    end
    table.insert(series, {
        name = "API Latency",
        tags = { service = "api-gateway", region = "us-east-1" },
        points = api_points
    })

    -- Series 2: Database Latency (trending upward with spike)
    local db_points = {}
    for i = 60, 1, -1 do
        local timestamp = now - (i * 60)
        local base = 20 + (60 - i) * 0.5  -- Gradual upward trend
        local value = base + math.random(-5, 10)
        -- Add a spike around the 20-minute mark
        if i >= 18 and i <= 22 then
            value = value + 40
        end
        table.insert(db_points, { timestamp = timestamp, value = value })
    end
    table.insert(series, {
        name = "Database Latency",
        tags = { service = "postgres", region = "us-east-1" },
        points = db_points
    })

    -- Series 3: Cache Hit Latency (low and stable)
    local cache_points = {}
    for i = 60, 1, -1 do
        local timestamp = now - (i * 60)
        local value = 5 + math.random(-2, 3)
        table.insert(cache_points, { timestamp = timestamp, value = value })
    end
    table.insert(series, {
        name = "Cache Latency",
        tags = { service = "redis", region = "us-east-1" },
        points = cache_points
    })

    return series
end

-- Command to show the demo chart pane
enya.register_command("demo-chart", {
    description = "Show demo metrics chart"
}, function(args)
    -- Add the pane
    enya.add_chart_pane("demo-metrics")

    -- Populate with initial data
    local series = generate_demo_data()
    enya.set_chart_data("demo-metrics", { series = series })

    enya.notify("info", "Demo chart added with " .. #series .. " series")
    return true
end)

-- Command to update the chart data
enya.register_command("demo-chart-update", {
    description = "Update demo chart with new data"
}, function(args)
    local series = generate_demo_data()
    enya.set_chart_data("demo-metrics", { series = series })
    enya.notify("info", "Demo chart updated")
    return true
end)

-- Command to simulate an error
enya.register_command("demo-chart-error", {
    description = "Simulate a fetch error in demo chart"
}, function(args)
    enya.set_chart_data("demo-metrics", {
        error = "Connection timeout: unable to reach metrics server"
    })
    enya.notify("warn", "Simulated error shown in demo chart")
    return true
end)

-- Note: Custom keybindings are not yet processed by the input handler.
-- For now, use the command palette (:demo-chart) to invoke plugin commands.

-- Lifecycle hooks
function on_activate()
    enya.log("info", "Demo chart plugin activated! Type :demo-chart to try it.")
end

function on_deactivate()
    enya.log("info", "Demo chart plugin deactivated")
end
