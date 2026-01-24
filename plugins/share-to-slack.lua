-- Share to Slack Plugin
-- Allows sharing current pane context to Slack via webhooks.
--
-- Setup:
-- 1. Create a Slack incoming webhook: https://api.slack.com/messaging/webhooks
-- 2. Set SLACK_WEBHOOK_URL environment variable
--
-- Usage:
-- 1. Copy this file to ~/.config/enya/plugins/
-- 2. Restart Enya
-- 3. Focus on a pane you want to share
-- 4. Type :share-slack [message] to share to Slack
-- 5. Type :share-clipboard to copy context to clipboard
--
-- Example:
--   :share-slack Check out this latency spike!
--   :share-clipboard

plugin = {
    name = "share-to-slack",
    version = "0.1.0",
    description = "Share pane context to Slack"
}

-- Configuration
local config = {
    slack_webhook = os.getenv("SLACK_WEBHOOK_URL"),
}

-- Simple JSON encoder for Lua tables
-- Handles strings, numbers, booleans, and nested tables/arrays
local function json_encode(value)
    if type(value) == "string" then
        -- Escape special characters
        local escaped = value:gsub('\\', '\\\\')
            :gsub('"', '\\"')
            :gsub('\n', '\\n')
            :gsub('\r', '\\r')
            :gsub('\t', '\\t')
        return '"' .. escaped .. '"'
    elseif type(value) == "number" then
        return tostring(value)
    elseif type(value) == "boolean" then
        return value and "true" or "false"
    elseif type(value) == "nil" then
        return "null"
    elseif type(value) == "table" then
        -- Check if it's an array (has sequential integer keys starting at 1)
        local is_array = true
        local max_index = 0
        for k, _ in pairs(value) do
            if type(k) == "number" and k > 0 and math.floor(k) == k then
                max_index = math.max(max_index, k)
            else
                is_array = false
                break
            end
        end
        is_array = is_array and max_index == #value

        if is_array then
            local parts = {}
            for _, v in ipairs(value) do
                table.insert(parts, json_encode(v))
            end
            return "[" .. table.concat(parts, ",") .. "]"
        else
            local parts = {}
            for k, v in pairs(value) do
                if type(k) == "string" then
                    table.insert(parts, json_encode(k) .. ":" .. json_encode(v))
                end
            end
            return "{" .. table.concat(parts, ",") .. "}"
        end
    end
    return "null"
end

-- Format time range as human-readable string
local function format_time_range()
    local tr = enya.get_time_range()
    if not tr then
        return "unknown time range"
    end

    local duration = tr["end"] - tr.start
    if duration < 60 then
        return string.format("last %d seconds", math.floor(duration))
    elseif duration < 3600 then
        return string.format("last %d minutes", math.floor(duration / 60))
    elseif duration < 86400 then
        return string.format("last %d hours", math.floor(duration / 3600))
    else
        return string.format("last %d days", math.floor(duration / 86400))
    end
end

-- Build context message from focused pane
local function build_context_message(user_message)
    local pane = enya.get_focused_pane()
    local time_range = format_time_range()

    local parts = {}

    -- Add user message if provided
    if user_message and user_message ~= "" then
        table.insert(parts, user_message)
        table.insert(parts, "")
    end

    -- Add pane context
    if pane then
        local pane_desc = "Pane: " .. (pane.title or pane.pane_type)
        table.insert(parts, pane_desc)

        if pane.query then
            table.insert(parts, "Query: `" .. pane.query .. "`")
        end

        if pane.metric_name then
            table.insert(parts, "Metric: " .. pane.metric_name)
        end
    else
        table.insert(parts, "No pane focused")
    end

    table.insert(parts, "Time range: " .. time_range)
    table.insert(parts, "")
    table.insert(parts, "_Shared from Enya_")

    return table.concat(parts, "\n")
end

-- Build Slack message payload
local function build_slack_payload(message)
    return json_encode({
        text = message,
        unfurl_links = false,
        unfurl_media = false
    })
end

-- Share to Slack
local function share_to_slack(args)
    if not config.slack_webhook then
        enya.notify("error", "SLACK_WEBHOOK_URL not set. Please set this environment variable.")
        return
    end

    local message = build_context_message(args)
    local payload = build_slack_payload(message)

    local resp = enya.http_post(config.slack_webhook, payload, {
        ["Content-Type"] = "application/json"
    })

    if resp.error then
        enya.notify("error", "Failed to share to Slack: " .. resp.error)
    elseif resp.status >= 400 then
        enya.notify("error", "Slack returned error: " .. (resp.body or "unknown"))
    else
        enya.notify("info", "Shared to Slack!")
    end
end

-- Copy context to clipboard
local function share_to_clipboard(_args)
    local message = build_context_message(nil)
    if enya.clipboard_write(message) then
        enya.notify("info", "Context copied to clipboard!")
    else
        enya.notify("error", "Failed to copy to clipboard")
    end
end

-- Register commands
enya.register_command("share-slack", {
    description = "Share current pane context to Slack",
    aliases = { "slack" },
    accepts_args = true
}, share_to_slack)

enya.register_command("share-clipboard", {
    description = "Copy current pane context to clipboard",
    aliases = { "yank-context" },
    accepts_args = false
}, share_to_clipboard)

-- Keybindings (leader key sequences)
-- These assume Space is your leader key
enya.keymap("<leader>ss", "share-slack", "Share to Slack")
enya.keymap("<leader>sy", "share-clipboard", "Copy context to clipboard")

-- Lifecycle hooks
function on_activate()
    if config.slack_webhook then
        enya.log("info", "Share to Slack plugin ready")
    else
        enya.log("warn", "Share to Slack plugin: SLACK_WEBHOOK_URL not configured")
    end
end
