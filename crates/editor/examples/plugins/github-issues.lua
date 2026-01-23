-- GitHub Issues Plugin
-- Demonstrates fetching data via HTTP and displaying in a custom table pane
--
-- Usage:
-- 1. Copy this file to ~/.config/enya/plugins/
-- 2. Restart Enya
-- 3. Run :github-issues <owner>/<repo> (e.g., :github-issues rust-lang/rust)
-- 4. Press Space+g+i to show issues for the last fetched repo

plugin = {
    name = "github-issues",
    version = "0.1.0",
    description = "View GitHub issues in a custom table pane"
}

-- Register a custom table pane for GitHub issues
enya.register_table_pane("github-issues", {
    title = "GitHub Issues",
    refresh_interval = 0,  -- Manual refresh
    columns = {
        { name = "#", key = "number", width = 60 },
        { name = "Title", key = "title" },
        { name = "State", key = "state", width = 80 },
        { name = "Author", key = "author", width = 120 },
        { name = "Comments", key = "comments", width = 80 }
    }
})

-- Store the current repo for refresh
local current_repo = nil

-- Parse GitHub issues from JSON (simplified parser)
local function parse_issues_json(json_str)
    local rows = {}

    -- Very simple JSON array parser for GitHub issues
    -- In production, you'd use a proper JSON library
    for issue_block in json_str:gmatch('%{[^{}]*"number"%s*:%s*(%d+)[^{}]*%}') do
        -- This is a simplified parser - real implementation would be more robust
    end

    -- For demo purposes, let's parse key fields manually
    local i = 1
    for block in json_str:gmatch('%{[^{}]-"number"%s*:%s*(%d+)[^{}]-"title"%s*:%s*"([^"]*)"[^{}]-"state"%s*:%s*"([^"]*)"[^{}]-"user"%s*:%s*%{[^{}]-"login"%s*:%s*"([^"]*)"[^{}]-%}[^{}]-"comments"%s*:%s*(%d+)[^{}]-%}') do
        -- Pattern didn't work well, let's use a different approach
    end

    -- Fallback: extract issues one by one using patterns
    local pos = 1
    while pos < #json_str do
        local num_start = json_str:find('"number"%s*:%s*', pos)
        if not num_start then break end

        local num_end = json_str:find('[,}]', num_start + 10)
        local number = json_str:sub(num_start + 9, num_end - 1):match('%d+')

        local title_start = json_str:find('"title"%s*:%s*"', pos)
        if title_start then
            local title_end = json_str:find('"', title_start + 10)
            local title = json_str:sub(title_start + 9, title_end - 1)

            local state_start = json_str:find('"state"%s*:%s*"', pos)
            local state_end = json_str:find('"', state_start + 10)
            local state = json_str:sub(state_start + 9, state_end - 1)

            local login_start = json_str:find('"login"%s*:%s*"', pos)
            local login_end = json_str:find('"', login_start + 10)
            local author = login_start and json_str:sub(login_start + 10, login_end - 1) or "unknown"

            local comments_start = json_str:find('"comments"%s*:%s*', pos)
            local comments_end = json_str:find('[,}]', comments_start + 12)
            local comments = comments_start and json_str:sub(comments_start + 12, comments_end - 1):match('%d+') or "0"

            if number and title then
                table.insert(rows, {
                    number = "#" .. number,
                    title = title:sub(1, 60),  -- Truncate long titles
                    state = state or "open",
                    author = author,
                    comments = comments or "0"
                })
            end
        end

        pos = num_start + 50  -- Move forward to find next issue
        if #rows >= 25 then break end  -- Limit to 25 issues
    end

    return rows
end

-- Fetch issues for a repository
local function fetch_issues(repo)
    if not repo or repo == "" then
        enya.set_pane_data("github-issues", {
            error = "No repository specified. Use :github-issues owner/repo"
        })
        return false
    end

    -- Validate repo format
    if not repo:match("^[%w%-_]+/[%w%-_%.]+$") then
        enya.set_pane_data("github-issues", {
            error = "Invalid repo format. Use: owner/repo"
        })
        return false
    end

    enya.notify("info", "Fetching issues from " .. repo .. "...")

    local url = "https://api.github.com/repos/" .. repo .. "/issues?state=all&per_page=25"
    local response = enya.http_get(url, {
        ["Accept"] = "application/vnd.github.v3+json",
        ["User-Agent"] = "Enya-Plugin"
    })

    if response.error then
        enya.set_pane_data("github-issues", {
            error = "Failed to fetch: " .. response.error
        })
        return false
    end

    if response.status ~= 200 then
        enya.set_pane_data("github-issues", {
            error = "GitHub API error: HTTP " .. tostring(response.status)
        })
        return false
    end

    local rows = parse_issues_json(response.body)

    if #rows == 0 then
        enya.set_pane_data("github-issues", {
            error = "No issues found or failed to parse response"
        })
        return false
    end

    enya.set_pane_data("github-issues", { rows = rows })
    enya.notify("info", "Loaded " .. #rows .. " issues from " .. repo)
    current_repo = repo
    return true
end

-- Command to fetch and show GitHub issues
enya.register_command("github-issues", {
    description = "Show GitHub issues for a repository (owner/repo)",
    accepts_args = true
}, function(args)
    if args and args ~= "" then
        current_repo = args
    end

    if not current_repo then
        enya.notify("error", "Usage: :github-issues owner/repo")
        return false
    end

    -- Add the pane if not already visible
    enya.add_custom_pane("github-issues")

    -- Fetch issues
    return fetch_issues(current_repo)
end)

-- Command to refresh current issues
enya.register_command("github-refresh", {
    description = "Refresh GitHub issues"
}, function(args)
    if not current_repo then
        enya.notify("error", "No repository set. Use :github-issues owner/repo first")
        return false
    end
    return fetch_issues(current_repo)
end)

-- Keybindings
enya.keymap("Space+g+i", "github-issues", "Show GitHub issues")
enya.keymap("Space+g+r", "github-refresh", "Refresh GitHub issues")

function on_activate()
    enya.log("info", "GitHub Issues plugin ready. Use :github-issues owner/repo")
end
