-- GitHub Actions Workflows Plugin
-- Displays workflow runs from GitHub Actions in a custom table pane
-- with a stat pane showing success rate.
--
-- Setup:
-- 1. Set GITHUB_TOKEN environment variable, or
-- 2. Have GitHub CLI (gh) authenticated (reads from ~/.config/gh/hosts.yml)
--
-- Usage:
-- 1. Copy this file to ~/.enya/plugins/
-- 2. Restart Enya
-- 3. Type :gh-actions <owner/repo> to show workflows (e.g., :gh-actions anthropics/claude-code)
-- 4. Type :gh-refresh to refresh the data

plugin = {
    name = "github-actions",
    version = "0.1.0",
    description = "Display GitHub Actions workflow runs"
}

-- Configuration
local config = {
    -- Default repository (can be overridden via command)
    repo = nil,
    -- Number of runs to fetch
    per_page = 25,
    -- GitHub token (set from environment or gh CLI)
    token = nil,
}

-- Forward declaration for refresh callback
local refresh_data

-- Register custom table pane for workflow runs with auto-refresh
enya.register_table_pane("gh-workflows", {
    title = "GitHub Actions",
    refresh_interval = 30,  -- Auto-refresh every 30 seconds
    columns = {
        { name = "Workflow", key = "workflow", width = 150 },
        { name = "Status", key = "status", width = 100 },
        { name = "Branch", key = "branch", width = 120 },
        { name = "Event", key = "event", width = 90 },
        { name = "Duration", key = "duration", width = 80 },
        { name = "Actor", key = "actor", width = 100 },
        { name = "Started", key = "started" }
    }
}, function()
    -- Called automatically when pane is visible and refresh interval elapsed
    if config.repo then
        refresh_data()
    end
end)

-- Register stat pane for success rate with auto-refresh
-- (shares the same data fetch as the table, no separate callback needed)
enya.register_stat_pane("gh-success-rate", {
    title = "Success Rate",
    refresh_interval = 30,  -- Same interval as table
    unit = "%"
})

-- Try to get GitHub token from environment or gh CLI
local function get_github_token()
    -- First try environment variable
    local token = os.getenv("GITHUB_TOKEN")
    if token and token ~= "" then
        return token
    end

    -- Try reading from gh CLI config
    local home = os.getenv("HOME")
    if home then
        local gh_config_path = home .. "/.config/gh/hosts.yml"
        local file = io.open(gh_config_path, "r")
        if file then
            local content = file:read("*all")
            file:close()
            -- Simple YAML parsing for oauth_token
            local token_match = content:match("oauth_token:%s*([^\n]+)")
            if token_match then
                return token_match:gsub("^%s+", ""):gsub("%s+$", "")
            end
        end
    end

    return nil
end

-- Initialize token
config.token = get_github_token()

-- Simple JSON array/object parser for GitHub API responses
-- (handles the specific structure we need)
local function parse_json_string(str, pos)
    -- Skip opening quote
    pos = pos + 1
    local result = ""
    while pos <= #str do
        local c = str:sub(pos, pos)
        if c == '"' then
            return result, pos + 1
        elseif c == '\\' then
            pos = pos + 1
            local escaped = str:sub(pos, pos)
            if escaped == 'n' then result = result .. '\n'
            elseif escaped == 't' then result = result .. '\t'
            elseif escaped == 'r' then result = result .. '\r'
            else result = result .. escaped
            end
        else
            result = result .. c
        end
        pos = pos + 1
    end
    return result, pos
end

local function parse_json_value(str, pos)
    -- Skip whitespace
    while pos <= #str and str:sub(pos, pos):match("%s") do
        pos = pos + 1
    end

    if pos > #str then return nil, pos end

    local c = str:sub(pos, pos)

    if c == '"' then
        return parse_json_string(str, pos)
    elseif c == '{' then
        -- Object
        local obj = {}
        pos = pos + 1
        while pos <= #str do
            -- Skip whitespace
            while pos <= #str and str:sub(pos, pos):match("%s") do pos = pos + 1 end
            if str:sub(pos, pos) == '}' then return obj, pos + 1 end
            if str:sub(pos, pos) == ',' then pos = pos + 1 end
            -- Skip whitespace
            while pos <= #str and str:sub(pos, pos):match("%s") do pos = pos + 1 end
            if str:sub(pos, pos) == '}' then return obj, pos + 1 end
            -- Parse key
            local key
            key, pos = parse_json_string(str, pos)
            -- Skip colon
            while pos <= #str and str:sub(pos, pos):match("[%s:]") do pos = pos + 1 end
            -- Parse value
            local value
            value, pos = parse_json_value(str, pos)
            obj[key] = value
        end
        return obj, pos
    elseif c == '[' then
        -- Array
        local arr = {}
        pos = pos + 1
        while pos <= #str do
            -- Skip whitespace
            while pos <= #str and str:sub(pos, pos):match("%s") do pos = pos + 1 end
            if str:sub(pos, pos) == ']' then return arr, pos + 1 end
            if str:sub(pos, pos) == ',' then pos = pos + 1 end
            -- Skip whitespace
            while pos <= #str and str:sub(pos, pos):match("%s") do pos = pos + 1 end
            if str:sub(pos, pos) == ']' then return arr, pos + 1 end
            -- Parse value
            local value
            value, pos = parse_json_value(str, pos)
            table.insert(arr, value)
        end
        return arr, pos
    elseif c:match("%d") or c == '-' then
        -- Number
        local num_str = ""
        while pos <= #str and str:sub(pos, pos):match("[%d%.%-eE+]") do
            num_str = num_str .. str:sub(pos, pos)
            pos = pos + 1
        end
        return tonumber(num_str), pos
    elseif str:sub(pos, pos + 3) == "true" then
        return true, pos + 4
    elseif str:sub(pos, pos + 4) == "false" then
        return false, pos + 5
    elseif str:sub(pos, pos + 3) == "null" then
        return nil, pos + 4
    end

    return nil, pos + 1
end

local function parse_json(str)
    local value, _ = parse_json_value(str, 1)
    return value
end

-- Format duration from seconds
local function format_duration(seconds)
    if not seconds or seconds < 0 then return "-" end
    if seconds < 60 then return string.format("%ds", seconds) end
    if seconds < 3600 then return string.format("%dm %ds", math.floor(seconds / 60), seconds % 60) end
    return string.format("%dh %dm", math.floor(seconds / 3600), math.floor((seconds % 3600) / 60))
end

-- Format relative time
local function format_time_ago(iso_time)
    if not iso_time then return "-" end

    -- Parse ISO 8601 timestamp (simplified)
    local year, month, day, hour, min, sec = iso_time:match("(%d+)-(%d+)-(%d+)T(%d+):(%d+):(%d+)")
    if not year then return iso_time end

    local timestamp = os.time({
        year = tonumber(year),
        month = tonumber(month),
        day = tonumber(day),
        hour = tonumber(hour),
        min = tonumber(min),
        sec = tonumber(sec)
    })

    local now = os.time()
    local diff = now - timestamp

    if diff < 60 then return "just now" end
    if diff < 3600 then return string.format("%dm ago", math.floor(diff / 60)) end
    if diff < 86400 then return string.format("%dh ago", math.floor(diff / 3600)) end
    if diff < 604800 then return string.format("%dd ago", math.floor(diff / 86400)) end

    return string.format("%s/%s", month, day)
end

-- Map GitHub status to display status (for auto-coloring)
local function map_status(status, conclusion)
    if status == "completed" then
        if conclusion == "success" then return "Success"
        elseif conclusion == "failure" then return "Failed"
        elseif conclusion == "cancelled" then return "Cancelled"
        elseif conclusion == "skipped" then return "Skipped"
        elseif conclusion == "timed_out" then return "Timeout"
        else return conclusion or "Unknown"
        end
    elseif status == "in_progress" then
        return "Running"
    elseif status == "queued" then
        return "Queued"
    elseif status == "waiting" then
        return "Waiting"
    elseif status == "pending" then
        return "Pending"
    else
        return status or "Unknown"
    end
end

-- Fetch workflow runs from GitHub API
local function fetch_workflow_runs(repo)
    if not config.token then
        return nil, "GitHub token not found. Set GITHUB_TOKEN or authenticate with gh CLI."
    end

    local url = string.format(
        "https://api.github.com/repos/%s/actions/runs?per_page=%d",
        repo,
        config.per_page
    )

    local response = enya.http_get(url, {
        ["Authorization"] = "Bearer " .. config.token,
        ["Accept"] = "application/vnd.github.v3+json",
        ["User-Agent"] = "Enya-GitHub-Actions-Plugin"
    })

    if response.error then
        return nil, response.error
    end

    if response.status ~= 200 then
        return nil, string.format("GitHub API error: %d", response.status)
    end

    local data = parse_json(response.body)
    if not data then
        return nil, "Failed to parse GitHub API response"
    end

    return data.workflow_runs or {}, nil
end

-- Convert workflow runs to table rows
local function runs_to_rows(runs)
    local rows = {}
    for _, run in ipairs(runs) do
        -- Calculate duration
        local duration = "-"
        if run.created_at and run.updated_at and run.status == "completed" then
            local start_year, start_month, start_day, start_hour, start_min, start_sec =
                run.created_at:match("(%d+)-(%d+)-(%d+)T(%d+):(%d+):(%d+)")
            local end_year, end_month, end_day, end_hour, end_min, end_sec =
                run.updated_at:match("(%d+)-(%d+)-(%d+)T(%d+):(%d+):(%d+)")

            if start_year and end_year then
                local start_ts = os.time({
                    year = tonumber(start_year), month = tonumber(start_month),
                    day = tonumber(start_day), hour = tonumber(start_hour),
                    min = tonumber(start_min), sec = tonumber(start_sec)
                })
                local end_ts = os.time({
                    year = tonumber(end_year), month = tonumber(end_month),
                    day = tonumber(end_day), hour = tonumber(end_hour),
                    min = tonumber(end_min), sec = tonumber(end_sec)
                })
                duration = format_duration(end_ts - start_ts)
            end
        elseif run.status == "in_progress" then
            duration = "..."
        end

        table.insert(rows, {
            workflow = run.name or "Unknown",
            status = map_status(run.status, run.conclusion),
            branch = run.head_branch or "-",
            event = run.event or "-",
            duration = duration,
            actor = run.actor and run.actor.login or "-",
            started = format_time_ago(run.created_at)
        })
    end
    return rows
end

-- Calculate success rate from runs
local function calculate_success_rate(runs)
    local completed = 0
    local succeeded = 0

    for _, run in ipairs(runs) do
        if run.status == "completed" then
            completed = completed + 1
            if run.conclusion == "success" then
                succeeded = succeeded + 1
            end
        end
    end

    if completed == 0 then
        return 0, 0, 0
    end

    return (succeeded / completed) * 100, succeeded, completed
end

-- Refresh workflow data (assigned to forward declaration above)
refresh_data = function()
    if not config.repo then
        enya.set_pane_data("gh-workflows", {
            error = "No repository set. Use :gh-actions <owner/repo> first."
        })
        return false
    end

    local runs, err = fetch_workflow_runs(config.repo)
    if err then
        enya.set_pane_data("gh-workflows", { error = err })
        enya.set_stat_data("gh-success-rate", { error = err })
        return false
    end

    -- Update table
    local rows = runs_to_rows(runs)
    enya.set_pane_data("gh-workflows", { rows = rows })

    -- Update success rate stat
    local rate, succeeded, total = calculate_success_rate(runs)
    local sparkline = {}
    -- Build sparkline from recent runs (1 for success, 0 for failure)
    for i = math.min(#runs, 20), 1, -1 do
        local run = runs[i]
        if run.status == "completed" then
            if run.conclusion == "success" then
                table.insert(sparkline, 100)
            else
                table.insert(sparkline, 0)
            end
        end
    end

    enya.set_stat_data("gh-success-rate", {
        value = rate,
        sparkline = sparkline,
        change_value = nil,  -- Could calculate vs previous period
        thresholds = {
            { value = 0, color = "red" },
            { value = 70, color = "yellow" },
            { value = 90, color = "green" }
        }
    })

    enya.notify("info", string.format("Refreshed: %d runs, %.0f%% success (%d/%d)",
        #runs, rate, succeeded, total))
    return true
end

-- Command to show GitHub Actions workflows
enya.register_command("gh-actions", {
    description = "Show GitHub Actions workflows",
    aliases = {"gha"},
    accepts_args = true
}, function(args)
    -- Parse repository from args
    if args and args ~= "" then
        -- Validate repo format (owner/repo)
        if not args:match("^[%w%-%.]+/[%w%-%.]+$") then
            enya.notify("error", "Invalid repository format. Use: owner/repo")
            return false
        end
        config.repo = args
    end

    if not config.repo then
        enya.notify("error", "Usage: :gh-actions <owner/repo>")
        return false
    end

    if not config.token then
        enya.notify("error", "GitHub token not found. Set GITHUB_TOKEN environment variable.")
        return false
    end

    -- Add panes
    enya.add_custom_pane("gh-workflows")
    enya.add_stat_pane("gh-success-rate")

    -- Fetch initial data
    refresh_data()
    return true
end)

-- Command to refresh workflow data
enya.register_command("gh-refresh", {
    description = "Refresh GitHub Actions data",
    aliases = {"ghr"}
}, function(args)
    if not config.repo then
        enya.notify("error", "No repository set. Use :gh-actions <owner/repo> first.")
        return false
    end

    return refresh_data()
end)

-- Command to set repository
enya.register_command("gh-repo", {
    description = "Set GitHub repository for Actions",
    accepts_args = true
}, function(args)
    if not args or args == "" then
        if config.repo then
            enya.notify("info", "Current repo: " .. config.repo)
        else
            enya.notify("info", "No repository set")
        end
        return true
    end

    if not args:match("^[%w%-%.]+/[%w%-%.]+$") then
        enya.notify("error", "Invalid repository format. Use: owner/repo")
        return false
    end

    config.repo = args
    enya.notify("info", "Repository set to: " .. config.repo)
    return true
end)

-- Lifecycle hooks
function on_activate()
    if config.token then
        enya.log("info", "GitHub Actions plugin activated. Token found.")
    else
        enya.log("warn", "GitHub Actions plugin activated. No token found - set GITHUB_TOKEN.")
    end
end

function on_deactivate()
    enya.log("info", "GitHub Actions plugin deactivated")
end
