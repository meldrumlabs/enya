//! Text formatting utilities for UI display.
//!
//! Provides helper functions for normalizing and truncating text
//! for display in the editor UI.

/// Normalize unicode characters that may not render correctly in our font.
///
/// Replaces special dashes, quotes, and other symbols with ASCII equivalents.
pub fn normalize_unicode(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            // Various dash types → regular hyphen
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}' => '-',
            // Curly quotes → straight quotes
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
            // Ellipsis → keep as is (single char)
            '\u{2026}' => c,
            // Non-breaking space → regular space
            '\u{00A0}' => ' ',
            // Everything else unchanged
            _ => c,
        })
        .collect()
}

/// Truncate a string to fit within `max_len` characters, appending `...` if
/// truncated. Uses char boundaries so it is safe with multi-byte UTF-8.
pub fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        let truncated: String = s.chars().take(max_len - 3).collect();
        format!("{truncated}...")
    } else {
        s.chars().take(max_len).collect()
    }
}

/// Truncate text for display, taking only the first line.
///
/// If the first line exceeds `max_len`, it is truncated with an ellipsis.
#[cfg(not(target_arch = "wasm32"))]
pub fn truncate_first_line(text: &str, max_len: usize) -> String {
    let first_line = text.lines().next().unwrap_or(text);
    truncate_with_ellipsis(first_line, max_len)
}

/// Truncate a file path to show the suffix (filename with parent context).
///
/// Attempts to show as much of the path suffix as possible while staying
/// within `max_len` characters. For example:
/// - `.../components/overlay/agent_panel.rs`
#[cfg(not(target_arch = "wasm32"))]
pub fn truncate_path_suffix(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        return path.to_string();
    }

    // Try to show as much of the path suffix as possible
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 1 {
        // No slashes, just truncate normally — show the tail
        let suffix: String = path
            .chars()
            .rev()
            .take(max_len.saturating_sub(3))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        return format!("...{suffix}");
    }

    // Start from the filename and add parent directories until we hit the limit
    let mut result = String::new();
    for part in parts.iter().rev() {
        let candidate = if result.is_empty() {
            part.to_string()
        } else {
            format!("{part}/{result}")
        };

        if candidate.len() + 4 > max_len {
            // Adding this part would exceed the limit
            break;
        }
        result = candidate;
    }

    if result.len() < path.len() {
        format!(".../{result}")
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_unicode_dashes() {
        assert_eq!(normalize_unicode("a\u{2013}b"), "a-b"); // en-dash
        assert_eq!(normalize_unicode("a\u{2014}b"), "a-b"); // em-dash
    }

    #[test]
    fn test_normalize_unicode_quotes() {
        assert_eq!(normalize_unicode("\u{201C}hello\u{201D}"), "\"hello\"");
        assert_eq!(normalize_unicode("\u{2018}hi\u{2019}"), "'hi'");
    }

    #[test]
    fn test_normalize_unicode_nbsp() {
        assert_eq!(normalize_unicode("hello\u{00A0}world"), "hello world");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_truncate_first_line_short() {
        assert_eq!(truncate_first_line("hello", 10), "hello");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_truncate_first_line_long() {
        assert_eq!(
            truncate_first_line("hello world this is too long", 15),
            "hello world ..."
        );
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_truncate_first_line_multiline() {
        assert_eq!(truncate_first_line("first\nsecond\nthird", 20), "first");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_truncate_path_short() {
        assert_eq!(truncate_path_suffix("src/main.rs", 50), "src/main.rs");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_truncate_path_long() {
        let path = "crates/editor/src/components/overlay/agent_panel.rs";
        let result = truncate_path_suffix(path, 30);
        assert!(result.starts_with(".../"));
        assert!(result.len() <= 30);
        assert!(result.ends_with("agent_panel.rs"));
    }
}
