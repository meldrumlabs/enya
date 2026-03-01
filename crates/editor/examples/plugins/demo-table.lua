-- Demo Table Plugin
-- Demonstrates the custom table pane feature
--
-- Usage:
-- 1. Copy this file to ~/.enya/plugins/
-- 2. Restart Enya
-- 3. Type :demo-table to show the demo table
-- 4. Type :demo-refresh to refresh with new data

plugin = {
    name = "demo-table",
    version = "0.1.0",
    description = "Demo plugin showing custom table panes"
}

-- Register a custom table pane type during plugin load
enya.register_table_pane("demo-services", {
    title = "Service Status",
    refresh_interval = 0,  -- Manual refresh only (set > 0 for auto-refresh)
    columns = {
        { name = "Service", key = "service" },
        { name = "Status", key = "status", width = 100 },
        { name = "Latency", key = "latency", width = 80 },
        { name = "Requests/s", key = "rps", width = 100 }
    }
})

-- Sample service data (in a real plugin, you'd fetch this via HTTP)
-- Status values like "Healthy", "Degraded", "Error" automatically render as colored badges!
local function generate_demo_data()
    -- These service configs show different status badges:
    -- Green: healthy, ok, running, active, up, online
    -- Yellow: warning, degraded, pending, slow
    -- Red: error, failed, critical, down, offline, unhealthy
    -- Blue: info, unknown, maintenance, paused
    local service_data = {
        { name = "api-gateway",          status = "Healthy",     latency = "12ms",  rps = "1,247" },
        { name = "user-service",         status = "Running",     latency = "8ms",   rps = "892" },
        { name = "auth-service",         status = "Degraded",    latency = "156ms", rps = "234" },
        { name = "payment-service",      status = "Healthy",     latency = "23ms",  rps = "567" },
        { name = "notification-service", status = "Error",       latency = "-",     rps = "0" },
        { name = "search-service",       status = "Maintenance", latency = "-",     rps = "-" },
        { name = "cache-service",        status = "Warning",     latency = "89ms",  rps = "2,341" },
        { name = "logging-service",      status = "Active",      latency = "5ms",   rps = "12,456" },
    }

    local rows = {}
    for _, svc in ipairs(service_data) do
        table.insert(rows, {
            service = svc.name,
            status = svc.status,
            latency = svc.latency,
            rps = svc.rps
        })
    end

    return rows
end

-- Command to show the demo table pane
enya.register_command("demo-table", {
    description = "Show demo service status table"
}, function(args)
    -- Add the pane
    enya.add_custom_pane("demo-services")

    -- Populate with initial data
    local rows = generate_demo_data()
    enya.set_pane_data("demo-services", { rows = rows })

    enya.notify("info", "Demo table added with " .. #rows .. " services")
    return true
end)

-- Command to refresh the data
enya.register_command("demo-refresh", {
    description = "Refresh demo table data"
}, function(args)
    local rows = generate_demo_data()
    enya.set_pane_data("demo-services", { rows = rows })
    enya.notify("info", "Demo table refreshed")
    return true
end)

-- Command to simulate an error
enya.register_command("demo-error", {
    description = "Simulate a fetch error in demo table"
}, function(args)
    enya.set_pane_data("demo-services", {
        error = "Connection refused: unable to reach metrics server"
    })
    enya.notify("warn", "Simulated error shown in demo table")
    return true
end)

-- Note: Custom keybindings are not yet processed by the input handler.
-- For now, use the command palette (:demo-table) to invoke plugin commands.

-- Lifecycle hooks
function on_activate()
    enya.log("info", "Demo table plugin activated! Type :demo-table to try it.")
end

function on_deactivate()
    enya.log("info", "Demo table plugin deactivated")
end
