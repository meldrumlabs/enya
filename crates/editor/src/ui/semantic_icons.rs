//! Semantic icon mappings for the Enya editor.
//!
//! This module provides mini.nvim-style semantic icon mappings using Nerd Font icons.
//! Icons are organized by category and can be easily customized or extended.

use egui_nerdfonts::regular;

// ============================================================================
// Standardized Icon Sizes - use these constants for consistent sizing
// ============================================================================

/// Icon size for inline text (status bars, list items, small labels)
pub const SIZE_INLINE: f32 = 12.0;

/// Icon size for list item icons (finders, tree items)
pub const SIZE_ITEM: f32 = 14.0;

/// Icon size for section headers and prominent UI elements
pub const SIZE_HEADER: f32 = 16.0;

/// Icon size for large/hero icons (landing page shortcuts, empty states)
pub const SIZE_LARGE: f32 = 20.0;

/// Icon size for buttons and clickable actions
pub const SIZE_BUTTON: f32 = 14.0;

// ============================================================================
// Metric Type Icons - icons based on what the metric measures
// ============================================================================

/// Get an icon based on the metric name/type
pub fn metric_type_icon(metric_name: &str) -> &'static str {
    let name_lower = metric_name.to_lowercase();

    // Error/Failure metrics - check early to prioritize over generic "count"/"total"
    if name_lower.contains("error")
        || name_lower.contains("fail")
        || name_lower.contains("panic")
        || name_lower.contains("exception")
    {
        return regular::ALERT_CIRCLE;
    }

    // Memory/Size metrics - check early to prioritize "heap_bytes" etc.
    if name_lower.contains("memory")
        || name_lower.contains("bytes")
        || name_lower.contains("size")
        || name_lower.contains("heap")
        || name_lower.contains("buffer")
        || name_lower.contains("cache")
    {
        return regular::MEMORY;
    }

    // Duration/Latency metrics
    if name_lower.contains("duration")
        || name_lower.contains("latency")
        || name_lower.contains("time")
        || name_lower.contains("_ms")
        || name_lower.contains("_ns")
        || name_lower.contains("_us")
    {
        return regular::TIMER;
    }

    // Count/Counter metrics
    if name_lower.contains("count") || name_lower.contains("total") || name_lower.contains("num_") {
        return regular::COUNTER;
    }

    // Rate metrics (per second, etc.)
    if name_lower.contains("rate")
        || name_lower.contains("_per_")
        || name_lower.contains("throughput")
        || name_lower.contains("qps")
        || name_lower.contains("rps")
    {
        return regular::SPEEDOMETER;
    }

    // CPU/Utilization metrics
    if name_lower.contains("cpu")
        || name_lower.contains("utilization")
        || name_lower.contains("usage")
        || name_lower.contains("load")
    {
        return regular::CPU;
    }

    // Success/OK metrics
    if name_lower.contains("success") || name_lower.contains("_ok") || name_lower.contains("passed")
    {
        return regular::CHECK_CIRCLE;
    }

    // Queue/Backlog metrics
    if name_lower.contains("queue")
        || name_lower.contains("backlog")
        || name_lower.contains("pending")
        || name_lower.contains("waiting")
    {
        return regular::STACK;
    }

    // Connection/Network metrics
    if name_lower.contains("connection")
        || name_lower.contains("socket")
        || name_lower.contains("network")
        || name_lower.contains("tcp")
        || name_lower.contains("http")
    {
        return regular::CONNECTION;
    }

    // IO metrics
    if name_lower.contains("read")
        || name_lower.contains("write")
        || name_lower.contains("disk")
        || name_lower.contains("io_")
    {
        return regular::HARDDISK;
    }

    // Thread/Worker metrics
    if name_lower.contains("thread")
        || name_lower.contains("worker")
        || name_lower.contains("pool")
        || name_lower.contains("spawn")
    {
        return regular::LAYERS;
    }

    // Poll/Event metrics (Tokio-style)
    if name_lower.contains("poll")
        || name_lower.contains("wake")
        || name_lower.contains("park")
        || name_lower.contains("schedule")
    {
        return regular::SYNC;
    }

    // Gauge/Current value metrics
    if name_lower.contains("gauge")
        || name_lower.contains("current")
        || name_lower.contains("active")
        || name_lower.contains("alive")
    {
        return regular::GAUGE;
    }

    // Histogram/Distribution metrics
    if name_lower.contains("histogram")
        || name_lower.contains("percentile")
        || name_lower.contains("p50")
        || name_lower.contains("p95")
        || name_lower.contains("p99")
    {
        return regular::CHART_BAR;
    }

    // Default: generic chart
    regular::CHART_LINE
}

// ============================================================================
// Category Icons - icons for metric categories/groups
// ============================================================================

/// Icons for metric categories (matching metrics_tree.rs categories)
pub mod category {
    use egui_nerdfonts::regular;

    pub const TOKIO: &str = regular::LIGHTNING_BOLT;
    pub const TASKS: &str = regular::CLIPBOARD_LIST;
    pub const DATAFUSION: &str = regular::DATABASE_1;
    pub const SYSTEM: &str = regular::CPU;
    pub const APPLICATION: &str = regular::CUBE;
    pub const OTHER: &str = regular::DOTS_HORIZONTAL;

    /// Get category icon from category name
    pub fn from_name(name: &str) -> &'static str {
        match name.to_lowercase().as_str() {
            "tokio" | "tokio runtime" => TOKIO,
            "tasks" | "task" => TASKS,
            "datafusion" => DATAFUSION,
            "system" => SYSTEM,
            "application" | "app" => APPLICATION,
            _ => OTHER,
        }
    }
}

// ============================================================================
// Status Icons - icons for various states and statuses
// ============================================================================

pub mod status {
    use egui_nerdfonts::regular;

    // Connection status
    pub const CONNECTED: &str = regular::WIFI;
    pub const DISCONNECTED: &str = regular::WIFI_OFF;
    pub const CONNECTING: &str = regular::WIFI_STRENGTH_2;
    pub const PLUG: &str = regular::POWER_PLUG;

    // Operation status
    pub const LOADING: &str = regular::LOADING;
    pub const SUCCESS: &str = regular::CHECK_CIRCLE;
    pub const ERROR: &str = regular::CLOSE_CIRCLE;
    pub const WARNING: &str = regular::WARNING;
    pub const INFO: &str = regular::INFORMATION;
    pub const QUESTION: &str = regular::HELP_CIRCLE;

    // Data status
    pub const EMPTY: &str = regular::CIRCLE_OUTLINE;
    pub const PARTIAL: &str = regular::CIRCLE_HALF;
    pub const COMPLETE: &str = regular::CIRCLE;

    // Mode indicators
    pub const NORMAL: &str = regular::CURSOR_DEFAULT;
    pub const INSERT: &str = regular::PENCIL;
    pub const VISUAL: &str = regular::SELECTION;
    pub const SEARCH: &str = regular::MAGNIFY;
    pub const COMMAND_MODE: &str = regular::TERMINAL;
}

// ============================================================================
// Navigation Icons - icons for UI navigation elements
// ============================================================================

pub mod nav {
    use egui_nerdfonts::regular;

    // Direction
    pub const LEFT: &str = regular::ARROW_LEFT;
    pub const RIGHT: &str = regular::ARROW_RIGHT;
    pub const UP: &str = regular::ARROW_UP;
    pub const DOWN: &str = regular::ARROW_DOWN;
    pub const COMPASS: &str = regular::COMPASS;

    // Expand/Collapse
    pub const EXPAND: &str = regular::CHEVRON_DOWN;
    pub const COLLAPSE: &str = regular::CHEVRON_RIGHT;
    pub const EXPAND_ALL: &str = regular::ARROW_EXPAND;
    pub const COLLAPSE_ALL: &str = regular::ARROW_COLLAPSE;

    // Layout
    pub const SIDEBAR: &str = regular::PAGE_LAYOUT_SIDEBAR_LEFT;
    pub const SPLIT_HORIZONTAL: &str = regular::ARROW_SPLIT_HORIZONTAL;
    pub const SPLIT_VERTICAL: &str = regular::ARROW_SPLIT_VERTICAL;
    pub const GRID: &str = regular::VIEW_GRID;
    pub const PANES: &str = regular::VIEW_GRID;
    pub const TABS: &str = regular::TAB;
    pub const FULLSCREEN: &str = regular::FULLSCREEN;
    pub const EXIT_FULLSCREEN: &str = regular::FULLSCREEN_EXIT;
    pub const TREE: &str = regular::SITEMAP;

    // Navigation targets
    pub const HOME: &str = regular::HOME;
    pub const SETTINGS: &str = regular::COG_1;
    pub const HELP: &str = regular::HELP_CIRCLE;
    pub const BACK: &str = regular::ARROW_LEFT;
    pub const FORWARD: &str = regular::ARROW_RIGHT;
}

// ============================================================================
// Action Icons - icons for user actions/operations
// ============================================================================

pub mod action {
    use egui_nerdfonts::regular;

    // CRUD operations
    pub const ADD: &str = regular::PLUS;
    pub const REMOVE: &str = regular::MINUS;
    pub const EDIT: &str = regular::PENCIL;
    pub const DELETE: &str = regular::TRASH;
    pub const SAVE: &str = regular::CONTENT_SAVE;
    pub const COPY: &str = regular::CONTENT_COPY;
    pub const PASTE: &str = regular::CLIPBOARD;
    pub const CONFIRM: &str = regular::CHECK;
    pub const CANCEL: &str = regular::CLOSE;

    // View operations
    pub const REFRESH: &str = regular::REFRESH;
    pub const RELOAD: &str = regular::RELOAD;
    pub const SEARCH: &str = regular::MAGNIFY;
    pub const FILTER: &str = regular::FILTER;
    pub const SORT: &str = regular::SORT;
    pub const ZOOM_IN: &str = regular::MAGNIFY_PLUS;
    pub const ZOOM_OUT: &str = regular::MAGNIFY_MINUS;
    pub const SWAP: &str = regular::SWAP_HORIZONTAL;
    pub const EXPLORE: &str = regular::COMPASS;
    pub const TAG: &str = regular::TAG;
    pub const SPLIT: &str = regular::ARROW_SPLIT_HORIZONTAL;
    pub const HELP: &str = regular::HELP_CIRCLE;
    pub const HISTORY: &str = regular::HISTORY;

    // Chart/Data operations
    pub const CHART: &str = regular::CHART_LINE;
    pub const ADD_CHART: &str = regular::CHART_TIMELINE_VARIANT;
    pub const EXPORT: &str = regular::EXPORT;
    pub const IMPORT: &str = regular::DOWNLOAD;
    pub const SHARE: &str = regular::SHARE;
    pub const LINK: &str = regular::LINK;

    // Control operations
    pub const PLAY: &str = regular::PLAY;
    pub const PAUSE: &str = regular::PAUSE;
    pub const STOP: &str = regular::STOP;
    pub const CLOSE: &str = regular::CLOSE;

    // AI/Agent operations
    pub const ROBOT: &str = regular::ROBOT;
    pub const BRAIN: &str = regular::BRAIN;
    pub const TOOL: &str = regular::WRENCH;
}

// ============================================================================
// File/Object Icons - icons for different object types
// ============================================================================

pub mod file {
    use egui_nerdfonts::regular;

    // File types
    pub const GENERIC: &str = regular::FILE_1;
    pub const TEXT: &str = regular::FILE_DOCUMENT;
    pub const CODE: &str = regular::FILE_CODE;
    pub const CONFIG: &str = regular::I_SETI_CONFIG;
    pub const DATA: &str = regular::FILE_TABLE;
    pub const IMAGE: &str = regular::FILE_IMAGE;

    // Folder
    pub const FOLDER: &str = regular::FOLDER_1;
    pub const FOLDER_OPEN: &str = regular::FOLDER_OPEN;
    pub const FOLDER_PLUS: &str = regular::FOLDER_PLUS;

    // Special items
    pub const QUERY: &str = regular::CODE_BRACES;
    pub const METRIC: &str = regular::CHART_LINE;
    pub const TAG: &str = regular::TAG;
    pub const LABEL: &str = regular::HASH;
    pub const TREE: &str = regular::SITEMAP;
    pub const GIT: &str = regular::GIT;
}

// ============================================================================
// Diff/Comparison Icons - icons for diff mode
// ============================================================================

pub mod diff {
    use egui_nerdfonts::regular;

    pub const DIFF: &str = regular::FILE_COMPARE;
    pub const ADDED: &str = regular::PLUS_CIRCLE;
    pub const REMOVED: &str = regular::MINUS_CIRCLE;
    pub const CHANGED: &str = regular::SWAP_HORIZONTAL;
    pub const UNCHANGED: &str = regular::EQUAL;
    pub const COMPARE: &str = regular::COMPARE;
}

// ============================================================================
// Git Icons - icons for git-related features
// ============================================================================

pub mod git {
    use egui_nerdfonts::regular;

    pub const BRANCH: &str = regular::GIT_BRANCH;
    pub const COMMIT: &str = regular::GIT_COMMIT;
    pub const MERGE: &str = regular::GIT_MERGE;
    pub const PULL_REQUEST: &str = regular::GIT_PULL_REQUEST;
    pub const DIFF: &str = regular::FILE_COMPARE;
    pub const FORK: &str = regular::SOURCE_FORK;
}

// ============================================================================
// Diagnostic Icons - icons for diagnostics and logging
// ============================================================================

pub mod diagnostic {
    use egui_nerdfonts::regular;

    pub const ERROR: &str = regular::CLOSE_CIRCLE;
    pub const WARNING: &str = regular::WARNING;
    pub const INFO: &str = regular::INFORMATION;
    pub const HINT: &str = regular::LIGHTBULB;
    pub const DEBUG: &str = regular::BUG;
    pub const TRACE: &str = regular::FOOTPRINT;
}

// ============================================================================
// Editor Mode Icons - icons for vim-style modes
// ============================================================================

pub mod mode {
    use egui_nerdfonts::regular;

    pub const NORMAL: &str = regular::CURSOR_DEFAULT;
    pub const INSERT: &str = regular::PENCIL;
    pub const VISUAL: &str = regular::SELECTION;
    pub const VISUAL_LINE: &str = regular::LIST;
    pub const VISUAL_BLOCK: &str = regular::VIEW_GRID;
    pub const COMMAND: &str = regular::TERMINAL;
    pub const SEARCH: &str = regular::MAGNIFY;
    pub const REPLACE: &str = regular::FIND_REPLACE;
    pub const VIEW: &str = regular::EYE;
}

// ============================================================================
// Keyboard Icons - icons for keyboard shortcuts display
// ============================================================================

pub mod keyboard {
    use egui_nerdfonts::regular;

    pub const KEYBOARD: &str = regular::KEYBOARD;
    pub const COMMAND_KEY: &str = regular::APPLE_KEYBOARD_COMMAND;
    pub const OPTION_KEY: &str = regular::APPLE_KEYBOARD_OPTION;
    pub const SHIFT_KEY: &str = regular::APPLE_KEYBOARD_SHIFT;
    pub const CONTROL_KEY: &str = regular::APPLE_KEYBOARD_CONTROL;
    pub const ENTER_KEY: &str = regular::KEYBOARD_RETURN;
    pub const ESCAPE_KEY: &str = regular::KEYBOARD_ESC;
    pub const TAB_KEY: &str = regular::KEYBOARD_TAB;
}

// ============================================================================
// Time Icons - icons for time/date related features
// ============================================================================

pub mod time {
    use egui_nerdfonts::regular;

    pub const CALENDAR: &str = regular::CALENDAR;
    pub const CLOCK: &str = regular::CLOCK;
    pub const TIMER: &str = regular::TIMER;
    pub const HISTORY: &str = regular::HISTORY;
}

// ============================================================================
// Completion Icons - icons for autocomplete item types
// ============================================================================

pub mod completion {
    use egui_nerdfonts::regular;

    pub const KEYWORD: &str = regular::FORMAT_TEXT;
    pub const OPERATOR: &str = regular::CODE_BRACES;
    pub const TAG_KEY: &str = regular::TAG;
    pub const TAG_VALUE: &str = regular::HASH;
    pub const FUNCTION: &str = regular::FUNCTION;
    pub const DURATION: &str = regular::TIMER;
    pub const METRIC: &str = regular::GAUGE;
}

// ============================================================================
// Empty State Icons - icons for empty/placeholder states
// ============================================================================

pub mod empty {
    use egui_nerdfonts::regular;

    pub const NO_DATA: &str = regular::CHART_LINE;
    pub const NO_RESULTS: &str = regular::MAGNIFY;
    pub const NO_ITEMS: &str = regular::INBOX;
    pub const NO_PLOTS: &str = regular::CHART_LINE;
    pub const NO_WORKSPACES: &str = regular::FOLDER_1;
    pub const NO_QUERIES: &str = regular::CODE_BRACES;
    pub const NO_METRICS: &str = regular::GAUGE;
}

// ============================================================================
// Statusline Icons - icons specific to status bar
// ============================================================================

pub mod statusline {
    use egui_nerdfonts::regular;

    pub const SEPARATOR: &str = regular::SLASH_FORWARD;
    pub const RECORDING: &str = regular::RECORD;
    pub const CLOCK: &str = regular::CLOCK_OUTLINE;
    pub const REFRESH: &str = regular::SYNC;
}

// ============================================================================
// Language Icons - icons for programming languages
// ============================================================================

pub mod language {
    use egui_nerdfonts::regular;

    // Language icons from MDI (Material Design Icons) - the LANGUAGE_* variants
    // include the actual language logos (Rust gear, Go gopher, Python snake, etc.)
    pub const RUST: &str = regular::LANGUAGE_RUST;
    pub const GO: &str = regular::LANGUAGE_GO;
    pub const PYTHON: &str = regular::LANGUAGE_PYTHON;
    pub const JAVASCRIPT: &str = regular::LANGUAGE_JAVASCRIPT;
    pub const TYPESCRIPT: &str = regular::LANGUAGE_TYPESCRIPT;
    pub const JAVA: &str = regular::LANGUAGE_JAVA;
    pub const C: &str = regular::LANGUAGE_C;
    pub const CPP: &str = regular::LANGUAGE_CPP;
    pub const CSHARP: &str = regular::LANGUAGE_CSHARP;
    pub const HTML: &str = regular::LANGUAGE_HTML5;
    pub const CSS: &str = regular::LANGUAGE_CSS3;
    pub const RUBY: &str = regular::LANGUAGE_RUBY;
    pub const PHP: &str = regular::LANGUAGE_PHP;
    pub const LUA: &str = regular::LANGUAGE_LUA;
    pub const KOTLIN: &str = regular::LANGUAGE_KOTLIN;
    pub const SWIFT: &str = regular::LANGUAGE_SWIFT;
    pub const MARKDOWN: &str = regular::LANGUAGE_MARKDOWN;

    /// Get language icon from language name
    pub fn from_name(name: &str) -> Option<&'static str> {
        match name.to_lowercase().as_str() {
            "rust" | "rs" => Some(RUST),
            "go" | "golang" => Some(GO),
            "python" | "py" => Some(PYTHON),
            "javascript" | "js" => Some(JAVASCRIPT),
            "typescript" | "ts" => Some(TYPESCRIPT),
            "java" => Some(JAVA),
            "c" => Some(C),
            "cpp" | "c++" | "cc" | "cxx" => Some(CPP),
            "csharp" | "cs" | "c#" => Some(CSHARP),
            "html" | "htm" => Some(HTML),
            "css" | "scss" | "sass" => Some(CSS),
            "ruby" | "rb" => Some(RUBY),
            "php" => Some(PHP),
            "lua" => Some(LUA),
            "kotlin" | "kt" => Some(KOTLIN),
            "swift" => Some(SWIFT),
            "markdown" | "md" => Some(MARKDOWN),
            _ => None,
        }
    }

    /// Get language icon from file extension
    pub fn from_extension(ext: &str) -> Option<&'static str> {
        match ext.to_lowercase().as_str() {
            "rs" => Some(RUST),
            "go" => Some(GO),
            "py" | "pyw" | "pyi" => Some(PYTHON),
            "js" | "mjs" | "cjs" => Some(JAVASCRIPT),
            "ts" | "tsx" => Some(TYPESCRIPT),
            "jsx" => Some(JAVASCRIPT),
            "java" => Some(JAVA),
            "c" | "h" => Some(C),
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some(CPP),
            "cs" => Some(CSHARP),
            "html" | "htm" => Some(HTML),
            "css" | "scss" | "sass" | "less" => Some(CSS),
            "rb" | "rake" => Some(RUBY),
            "php" => Some(PHP),
            "lua" => Some(LUA),
            "kt" | "kts" => Some(KOTLIN),
            "swift" => Some(SWIFT),
            "md" | "markdown" => Some(MARKDOWN),
            _ => None,
        }
    }
}

/// Get a file icon based on file path/extension
pub fn file_icon(path: &std::path::Path) -> &'static str {
    // Try to get language-specific icon first
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if let Some(icon) = language::from_extension(ext) {
            return icon;
        }

        // Check for config files
        match ext.to_lowercase().as_str() {
            "toml" | "yaml" | "yml" | "json" | "ini" | "conf" | "cfg" => {
                return file::CONFIG;
            }
            "csv" | "tsv" | "parquet" | "avro" => return file::DATA,
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" => return file::IMAGE,
            "txt" | "log" => return file::TEXT,
            _ => {}
        }
    }

    // Check for special filenames
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        let name_lower = name.to_lowercase();
        if name_lower.starts_with("dockerfile")
            || name_lower == "makefile"
            || name_lower == "cmakelists.txt"
            || name_lower == "cargo.toml"
            || name_lower == "package.json"
            || name_lower == "go.mod"
            || name_lower == ".gitignore"
            || name_lower == ".env"
        {
            return file::CONFIG;
        }
        if name_lower == "readme.md" || name_lower == "changelog.md" || name_lower == "license" {
            return file::TEXT;
        }
    }

    // Default to code icon for unknown types
    file::CODE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_type_icon_duration() {
        assert_eq!(metric_type_icon("request_duration_ms"), regular::TIMER);
        assert_eq!(metric_type_icon("latency_p99"), regular::TIMER);
        assert_eq!(metric_type_icon("poll_time_ns"), regular::TIMER);
    }

    #[test]
    fn test_metric_type_icon_count() {
        assert_eq!(metric_type_icon("request_count"), regular::COUNTER);
        assert_eq!(metric_type_icon("total_requests"), regular::COUNTER);
        assert_eq!(metric_type_icon("num_connections"), regular::COUNTER);
    }

    #[test]
    fn test_metric_type_icon_memory() {
        assert_eq!(metric_type_icon("heap_bytes"), regular::MEMORY);
        assert_eq!(metric_type_icon("memory_used"), regular::MEMORY);
        assert_eq!(metric_type_icon("buffer_size"), regular::MEMORY);
    }

    #[test]
    fn test_metric_type_icon_error() {
        assert_eq!(metric_type_icon("error_count"), regular::ALERT_CIRCLE);
        assert_eq!(metric_type_icon("panics_total"), regular::ALERT_CIRCLE);
    }

    #[test]
    fn test_metric_type_icon_default() {
        assert_eq!(metric_type_icon("some_random_metric"), regular::CHART_LINE);
    }

    #[test]
    fn test_category_from_name() {
        assert_eq!(category::from_name("tokio"), category::TOKIO);
        assert_eq!(category::from_name("Tokio Runtime"), category::TOKIO);
        assert_eq!(category::from_name("system"), category::SYSTEM);
        assert_eq!(category::from_name("unknown"), category::OTHER);
    }
}
