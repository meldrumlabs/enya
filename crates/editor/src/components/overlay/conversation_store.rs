//! Conversation thread management for the agent panel.
//!
//! Supports named threads, pinning, and disk persistence (native only).
//! On WASM, conversations are memory-only for the session.

use serde::{Deserialize, Serialize};

use crate::components::util::MessageRole;

/// A serializable message (role + content only, no inline blocks).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedMessage {
    pub role: MessageRole,
    pub content: String,
}

/// A named conversation thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationThread {
    /// Unique identifier (timestamp-based).
    pub id: String,
    /// User-visible name (auto-generated or renamed).
    pub name: String,
    /// Messages in this thread.
    pub messages: Vec<SavedMessage>,
    /// Whether this thread is pinned to the top.
    pub pinned: bool,
    /// Unix timestamp when created.
    pub created_at: i64,
    /// Unix timestamp when last modified.
    pub updated_at: i64,
}

impl Default for ConversationThread {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationThread {
    /// Create a new empty thread with an auto-generated name.
    pub fn new() -> Self {
        let now = crate::util::now_unix_secs();
        Self {
            id: format!("{now}"),
            name: "New conversation".to_string(),
            messages: Vec::new(),
            pinned: false,
            created_at: now,
            updated_at: now,
        }
    }

    /// Auto-name from the first user message (truncated to 40 chars).
    pub fn auto_name_from_messages(&mut self) {
        if let Some(msg) = self.messages.iter().find(|m| m.role == MessageRole::User) {
            let first_line = msg.content.lines().next().unwrap_or("").trim();
            if !first_line.is_empty() {
                self.name = crate::components::util::text_formatting::truncate_with_ellipsis(
                    first_line, 40,
                );
            }
        }
    }
}

/// Manages a collection of conversation threads with optional disk persistence.
pub struct ConversationStore {
    /// All threads, ordered by most recently updated.
    pub threads: Vec<ConversationThread>,
    /// Index of the currently active thread (None = no active thread).
    pub active_idx: Option<usize>,
    /// Whether the thread list popup is open.
    pub picker_open: bool,
    /// Whether rename mode is active for the current thread.
    pub renaming: bool,
    /// Buffer for the rename text input.
    pub rename_buf: String,
    /// Workspace name for scoped conversation storage (native only).
    #[cfg(not(target_arch = "wasm32"))]
    workspace_name: Option<String>,
    /// Project name for scoped conversation storage (native only).
    #[cfg(not(target_arch = "wasm32"))]
    project_name: Option<String>,
}

impl Default for ConversationStore {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl ConversationStore {
    pub fn new(
        #[cfg_attr(target_arch = "wasm32", allow(unused))] workspace_name: Option<String>,
        #[cfg_attr(target_arch = "wasm32", allow(unused))] project_name: Option<String>,
    ) -> Self {
        #[allow(unused_mut)]
        let mut store = Self {
            threads: Vec::new(),
            active_idx: None,
            picker_open: false,
            renaming: false,
            rename_buf: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            workspace_name,
            #[cfg(not(target_arch = "wasm32"))]
            project_name,
        };
        // Load saved threads on native
        #[cfg(not(target_arch = "wasm32"))]
        store.load_from_disk();
        store
    }

    /// Update the workspace and project name and reload conversations from the new location.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_conversation_scope(&mut self, workspace: Option<String>, project: Option<String>) {
        self.workspace_name = workspace;
        self.project_name = project;
        self.threads.clear();
        self.active_idx = None;
        self.load_from_disk();
    }

    /// Get the active thread, if any.
    pub fn active_thread(&self) -> Option<&ConversationThread> {
        self.active_idx.and_then(|i| self.threads.get(i))
    }

    /// Get the active thread mutably.
    pub fn active_thread_mut(&mut self) -> Option<&mut ConversationThread> {
        self.active_idx.and_then(|i| self.threads.get_mut(i))
    }

    /// Create a new thread and make it active. Returns the new thread's index.
    pub fn new_thread(&mut self) -> usize {
        let thread = ConversationThread::new();
        self.threads.insert(0, thread);
        // Adjust active_idx since we inserted at 0
        self.active_idx = Some(0);
        0
    }

    /// Switch to a thread by index.
    pub fn switch_to(&mut self, idx: usize) {
        if idx < self.threads.len() {
            self.active_idx = Some(idx);
        }
    }

    /// Toggle pin on a thread by index.
    pub fn toggle_pin(&mut self, idx: usize) {
        if let Some(thread) = self.threads.get_mut(idx) {
            thread.pinned = !thread.pinned;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let thread_id = self.threads.get(idx).map(|t| t.id.clone());
        self.sort_threads();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(id) = &thread_id {
            if let Some(thread) = self.threads.iter().find(|t| &t.id == id) {
                self.save_thread_file(thread);
            }
        }
    }

    /// Delete a thread by index.
    pub fn delete_thread(&mut self, idx: usize) {
        if idx >= self.threads.len() {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let id = self.threads[idx].id.clone();
        self.threads.remove(idx);
        // Fix active_idx
        match self.active_idx {
            Some(active) if active == idx => {
                self.active_idx = if self.threads.is_empty() {
                    None
                } else {
                    Some(active.min(self.threads.len() - 1))
                };
            }
            Some(active) if active > idx => {
                self.active_idx = Some(active - 1);
            }
            _ => {}
        }
        // Remove file on native
        #[cfg(not(target_arch = "wasm32"))]
        self.delete_file(&id);
    }

    /// Save the active thread to disk (native only).
    pub fn save_active(&mut self) {
        if let Some(thread) = self.active_thread_mut() {
            thread.updated_at = crate::util::now_unix_secs();
        }
        self.sort_threads();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(idx) = self.active_idx {
            if let Some(thread) = self.threads.get(idx) {
                let thread = thread.clone();
                self.save_thread_file(&thread);
            }
        }
    }

    /// Sort threads: pinned first, then by updated_at descending.
    fn sort_threads(&mut self) {
        // Remember active thread id
        let active_id = self.active_thread().map(|t| t.id.clone());
        self.threads.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then(b.updated_at.cmp(&a.updated_at))
        });
        // Restore active_idx
        if let Some(id) = active_id {
            self.active_idx = self.threads.iter().position(|t| t.id == id);
        }
    }

    // ── Native persistence ──────────────────────────────────────────

    #[cfg(not(target_arch = "wasm32"))]
    fn conversations_dir(&self) -> std::path::PathBuf {
        let project = self.project_name.as_deref().unwrap_or("_unknown");
        enya_config::project_conversations_dir(project, self.workspace_name.as_deref())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_disk(&mut self) {
        let dir = self.conversations_dir();
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(thread) = serde_json::from_str::<ConversationThread>(&data) {
                    self.threads.push(thread);
                }
            }
        }

        self.sort_threads();
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_thread_file(&self, thread: &ConversationThread) {
        let dir = self.conversations_dir();
        let path = dir.join(format!("{}.json", thread.id));
        match serde_json::to_string_pretty(thread) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&path, data) {
                    log::warn!("Failed to save conversation {}: {e}", thread.id);
                }
            }
            Err(e) => {
                log::warn!("Failed to serialize conversation {}: {e}", thread.id);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn delete_file(&self, id: &str) {
        let dir = self.conversations_dir();
        let path = dir.join(format!("{id}.json"));
        let _ = std::fs::remove_file(path);
    }
}
