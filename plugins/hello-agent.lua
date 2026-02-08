-- Hello Agent Plugin
-- A simple plugin demonstrating headless-friendly commands for AI agents.
--
-- Commands:
--   :greet [name]   — Print a greeting
--   :http-status    — Check an HTTP endpoint
--   :env [var]      — Read an environment variable

plugin = {
    name = "hello-agent",
    version = "0.1.0",
    description = "Demo plugin for headless CLI execution"
}

enya.register_command("greet", {
    description = "Print a greeting message",
    accepts_args = true,
}, function(args)
    local name = (args and args ~= "") and args or "world"
    enya.notify("info", "Hello, " .. name .. "! (from Lua plugin)")
    return true
end)

enya.register_command("http-status", {
    description = "Check HTTP status of a URL",
    accepts_args = true,
}, function(args)
    local url = (args and args ~= "") and args or "https://httpbin.org/status/200"
    enya.log("info", "Checking " .. url)
    local resp = enya.http_get(url, {})
    if resp.error then
        enya.notify("error", "Request failed: " .. resp.error)
        return false
    end
    enya.notify("info", url .. " returned status " .. tostring(resp.status))
    return true
end)

enya.register_command("env", {
    description = "Read an environment variable",
    accepts_args = true,
}, function(args)
    if not args or args == "" then
        enya.notify("error", "Usage: env <VARIABLE_NAME>")
        return false
    end
    local value = os.getenv(args)
    if value then
        enya.notify("info", args .. "=" .. value)
    else
        enya.notify("warn", args .. " is not set")
    end
    return true
end)

function on_activate()
    enya.log("info", "hello-agent plugin activated")
end
