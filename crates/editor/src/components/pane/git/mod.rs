//! PR Review pane — review GitHub pull requests inside Enya.
//!
//! Provides a full PR review experience: list open PRs, view diffs,
//! add comments, approve/request changes, and integrate with AI agents.

mod detail_view;
mod diff_view;
mod list_view;
mod walkthrough;

use rustc_hash::FxHashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::AsyncRuntime;
use crate::components::util::file_opener::FileOpenerPopup;
use crate::components::util::next_id_usize;
use crate::git::api::{
    self, CheckRun, DraftComment, IssueComment, MergeMethod, PrComment, PrFile, PrReview,
    PullRequest, ReviewEvent,
};
use crate::git::diff::FileDiff;
use crate::git::diff_renderer::{DiffKeyAction, DiffRenderer};
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// The current view state of the PR review pane.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrReviewView {
    /// Listing open pull requests.
    List,
    /// Viewing a specific PR's details.
    Detail,
}

/// Active segment in the PR list view inbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrListSegment {
    /// PRs that need the current user's review.
    NeedsReview,
    /// PRs authored by the current user.
    MyPrs,
    /// All open PRs.
    All,
}

/// Active tab in the detail view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DetailTab {
    Files,
    Conversation,
    Checks,
}

/// Direction for `]` / `[` bracket prefix keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BracketDir {
    Next,
    Prev,
}

/// Actions that the workspace needs to handle for the PR review pane.
#[derive(Debug, Clone)]
pub enum PrReviewPaneAction {
    /// No action.
    None,
}

/// Maximum number of PRs to preload after the list is fetched.
const PRELOAD_COUNT: usize = 10;

/// Aggregated review state for a PR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReviewState {
    Approved,
    ChangesRequested,
}

/// Result types for async operations.
type PrListResult = Result<Vec<PullRequest>, String>;
type PrDetailResult = Result<(PullRequest, Vec<PrFile>, String), String>;
type PrCommentsResult = Result<
    (
        Vec<PrComment>,
        Vec<IssueComment>,
        Vec<PrReview>,
        Vec<api::ReviewThreadState>,
    ),
    String,
>;
type PrChecksResult = Result<Vec<CheckRun>, String>;
type PrSubmitResult = Result<(), String>;
type PrMergeResult = Result<api::MergeOutcome, String>;
type SingleCommentResult = Result<PrComment, String>;
/// Result of a thread resolve/unresolve mutation: (thread_node_id, new_resolved_state).
type PrResolveResult = Result<(String, bool), String>;
type AvatarResult = (String, Result<Vec<u8>, String>);

/// All preloaded data for a single PR.
struct PreloadedPr {
    pr: PullRequest,
    files: Vec<PrFile>,
    file_diffs: Vec<FileDiff>,
    review_comments: Vec<PrComment>,
    issue_comments: Vec<IssueComment>,
    check_runs: Vec<CheckRun>,
    reviews: Vec<PrReview>,
}

/// Combined result from preloading a single PR.
type PreloadResult = Result<PreloadedPr, String>;

/// In-flight preload slot: (pr_number, result).
type PendingPreload = Arc<Mutex<Option<(u32, PreloadResult)>>>;

/// A pane for reviewing GitHub pull requests.
pub struct PrReviewPane {
    id: usize,
    name: String,
    theme: AppTheme,

    // GitHub repo info
    owner: String,
    repo: String,

    /// AI model ID from user settings (e.g. "claude-opus-4-6").
    ai_model: Option<String>,

    // Navigation
    view: PrReviewView,

    // ── List view state ──
    pull_requests: Vec<PullRequest>,
    selected_pr_index: usize,
    /// Whether to scroll the list to the selected PR (set on keyboard nav, cleared after render).
    list_scroll_to_selected: bool,
    list_loading: bool,
    list_error: Option<String>,
    pending_list: Arc<Mutex<Option<PrListResult>>>,
    /// Current inbox segment filter.
    list_segment: PrListSegment,
    /// Login of the authenticated user (fetched from /user when token is set).
    current_user_login: Option<String>,
    /// Pending async result for authenticated user fetch.
    pending_user: Arc<Mutex<Option<Result<crate::git::api::AuthenticatedUser, String>>>>,

    // ── Detail view state ──
    current_pr: Option<PullRequest>,
    pr_files: Vec<PrFile>,
    file_diffs: Vec<FileDiff>,
    review_comments: Vec<PrComment>,
    /// Cached comment threads, rebuilt when `review_comments` changes.
    cached_threads: Vec<crate::git::api::CommentThread>,
    issue_comments: Vec<IssueComment>,
    check_runs: Vec<CheckRun>,
    /// Reviews on the current PR (for showing approval state).
    reviews: Vec<PrReview>,
    /// Preloaded review data keyed by PR number (for list view badges).
    preloaded_reviews: FxHashMap<u32, Vec<PrReview>>,
    /// Preloaded merge-readiness: all checks pass, approved, mergeable, not draft.
    preloaded_merge_ready: FxHashMap<u32, bool>,
    /// Preloaded comment counts (review + issue comments) keyed by PR number.
    preloaded_comment_counts: FxHashMap<u32, usize>,
    selected_file_index: usize,
    active_tab: DetailTab,
    detail_loading: bool,
    detail_error: Option<String>,
    pending_detail: Arc<Mutex<Option<PrDetailResult>>>,
    pending_comments: Arc<Mutex<Option<PrCommentsResult>>>,
    pending_checks: Arc<Mutex<Option<PrChecksResult>>>,
    /// Shared diff renderer with search, selection, hunk navigation.
    diff_renderer: DiffRenderer,

    // ── Review draft ──
    pub(crate) draft_comments: Vec<DraftComment>,
    draft_body: String,
    /// (file_index, line_number) of the line being commented on.
    commenting_line: Option<(usize, usize)>,
    comment_input: String,
    /// Tracks which comment threads are collapsed, keyed by (file_path, line_number).
    collapsed_threads: rustc_hash::FxHashSet<(String, usize)>,
    /// Measured card heights from the previous frame, keyed by line_num.
    /// Used by floating-comment layout to avoid card overlap.
    floating_card_heights: rustc_hash::FxHashMap<usize, f32>,
    submitting_review: bool,
    submit_error: Option<String>,
    submit_success: Option<String>,
    pending_submit: Arc<Mutex<Option<PrSubmitResult>>>,
    pending_single_comment: Arc<Mutex<Option<SingleCommentResult>>>,

    // ── Submit review panel ──
    /// Whether the consolidated "Submit Review" panel is open.
    submit_panel_open: bool,
    /// Timestamp when the last success/error flash started (for animated button feedback).
    flash_start: Option<crate::util::Instant>,
    /// The flash type: true = success (green), false = error (red).
    flash_is_success: bool,
    /// Whether the auto-surface prompt has been dismissed for this PR.
    auto_surface_dismissed: bool,

    // ── Merge ──
    /// Whether the merge dropdown popup is open.
    merge_popup_open: bool,
    /// Selected merge strategy.
    merge_method: MergeMethod,
    /// Whether a merge request is in-flight.
    merging: bool,
    /// Async result of the merge request.
    pending_merge: Arc<Mutex<Option<PrMergeResult>>>,

    // ── AI Walkthrough ──
    /// Current walkthrough state.
    walkthrough_state: walkthrough::WalkthroughState,
    /// Receiver for streaming walkthrough AI events (native only).
    #[cfg(not(target_arch = "wasm32"))]
    walkthrough_receiver: Option<std::sync::mpsc::Receiver<enya_ai::AgentEvent>>,
    /// Accumulated text from the walkthrough AI response (streamed deltas).
    #[cfg(not(target_arch = "wasm32"))]
    walkthrough_response_text: String,

    // ── Description ──
    /// Whether the PR description card is collapsed in the main detail view.
    description_collapsed: bool,
    /// Whether the PR description is collapsed in the Conversation tab.
    conv_description_collapsed: bool,

    // ── File tree ──
    /// Collapsed directory paths in the file tree panel.
    collapsed_dirs: rustc_hash::FxHashSet<String>,
    /// Whether to scroll the file tree to make the selected file visible.
    file_tree_scroll_to_selected: bool,
    /// Whether the file panel is collapsed (hidden) for full-width diff view.
    file_panel_collapsed: bool,

    // ── Seen comments ──
    /// Comment IDs that the user has "seen" (by viewing the file containing them).
    seen_comment_ids: rustc_hash::FxHashSet<u64>,

    // ── Comment tree entries ──
    /// Files whose comment thread list is expanded in the file tree sidebar.
    expanded_comment_files: rustc_hash::FxHashSet<String>,

    // ── Thread resolution ──
    /// Thread resolution state keyed by (path, line). Populated from GitHub
    /// GraphQL and locally toggled; merged into cached_threads for rendering.
    resolved_thread_lines: rustc_hash::FxHashSet<(String, usize)>,
    /// Thread node IDs keyed by (path, line) — needed to call the GraphQL
    /// resolve/unresolve mutation.
    thread_node_ids: FxHashMap<(String, usize), String>,
    /// Filter toggle: when true, hide resolved threads in the sidebar tree.
    show_only_unresolved: bool,
    /// In-flight thread resolve/unresolve result: Ok(node_id, new_state) / Err.
    pending_resolve: Arc<Mutex<Option<PrResolveResult>>>,

    // ── Per-file reviewed status ──
    /// File paths the user has marked as "reviewed".
    reviewed_files: rustc_hash::FxHashSet<String>,

    // ── Markdown preview ──
    /// Whether markdown preview mode is active (renders the new file content as markdown).
    markdown_preview: bool,
    /// Scroll offset for markdown preview (managed manually for j/k keyboard scrolling).
    markdown_scroll_y: f32,
    /// Cached markdown content string for the current file (avoids rebuilding every frame).
    /// Tuple of (file_index, content).
    markdown_content_cache: Option<(usize, String)>,

    // ── File opener ──
    file_opener: FileOpenerPopup,
    /// Repo root for constructing full file paths.
    repo_root: Option<std::path::PathBuf>,
    /// Whether to open the file opener popup (deferred from keyboard).
    pending_open_file_opener: bool,

    // ── List filter ──
    /// Whether the `/` filter bar is active in list view.
    filter_active: bool,
    /// The current filter query string.
    filter_query: String,

    // ── Keyboard deferred actions ──
    /// PR number to open (set by keyboard, consumed next frame).
    pending_open_pr: Option<u32>,
    /// Whether to refresh the PR list (set by keyboard).
    pending_refresh: bool,
    /// Whether to go back to list view (set by keyboard).
    pending_go_back: bool,
    /// Pending `]` or `[` prefix for two-key sequences like `]c` / `[c`.
    bracket_pending: Option<BracketDir>,

    // ── Focus ──
    /// Whether this pane is the focused tile in the workspace.
    focused: bool,

    // ── Preload cache ──
    /// Cached preloaded PR data, keyed by PR number.
    preload_cache: FxHashMap<u32, PreloadedPr>,
    /// In-flight preload results.
    pending_preloads: Vec<PendingPreload>,
    /// Whether we have already kicked off preloading for the current list.
    preload_started: bool,

    // ── Avatar cache ──
    /// Cached avatar textures, keyed by GitHub login.
    avatar_textures: FxHashMap<String, egui::TextureHandle>,
    /// Logins for which avatar fetches are in-flight or completed.
    avatar_fetched: rustc_hash::FxHashSet<String>,
    /// Pending avatar fetch results: (login, image_bytes).
    pending_avatars: Arc<Mutex<Vec<AvatarResult>>>,

    // ── Async infrastructure ──
    http_client: reqwest::Client,
    async_runtime: AsyncRuntime,
    token: Option<String>,
    /// Counts consecutive frames where the token hasn't changed.
    /// Auto-fetch is deferred until the token is stable (≥2 frames) so the
    /// git credential token has a chance to arrive before firing requests.
    token_stable_frames: u8,
}

impl PrReviewPane {
    /// Create a new PR review pane for the given repository.
    pub fn new(owner: &str, repo: &str, async_runtime: AsyncRuntime) -> Self {
        Self {
            id: next_id_usize(),
            name: format!("PRs: {owner}/{repo}"),
            theme: AppTheme::default(),
            owner: owner.to_string(),
            repo: repo.to_string(),
            ai_model: None,
            view: PrReviewView::List,
            pull_requests: Vec::new(),
            selected_pr_index: 0,
            list_scroll_to_selected: false,
            list_loading: false,
            list_error: None,
            pending_list: Arc::new(Mutex::new(None)),
            list_segment: PrListSegment::NeedsReview,
            current_user_login: None,
            pending_user: Arc::new(Mutex::new(None)),
            current_pr: None,
            pr_files: Vec::new(),
            file_diffs: Vec::new(),
            review_comments: Vec::new(),
            cached_threads: Vec::new(),
            issue_comments: Vec::new(),
            check_runs: Vec::new(),
            reviews: Vec::new(),
            preloaded_reviews: FxHashMap::default(),
            preloaded_merge_ready: FxHashMap::default(),
            preloaded_comment_counts: FxHashMap::default(),
            selected_file_index: 0,
            active_tab: DetailTab::Files,
            detail_loading: false,
            detail_error: None,
            pending_detail: Arc::new(Mutex::new(None)),
            pending_comments: Arc::new(Mutex::new(None)),
            pending_checks: Arc::new(Mutex::new(None)),
            diff_renderer: {
                let mut r = DiffRenderer::new("pr_diff", typography::SM);
                r.set_allow_hunk_expansion(false);
                r
            },
            draft_comments: Vec::new(),
            draft_body: String::new(),
            commenting_line: None,
            comment_input: String::new(),
            collapsed_threads: rustc_hash::FxHashSet::default(),
            floating_card_heights: rustc_hash::FxHashMap::default(),
            submitting_review: false,
            submit_error: None,
            submit_success: None,
            pending_submit: Arc::new(Mutex::new(None)),
            pending_single_comment: Arc::new(Mutex::new(None)),
            submit_panel_open: false,
            flash_start: None,
            flash_is_success: false,
            auto_surface_dismissed: false,
            merge_popup_open: false,
            merge_method: MergeMethod::Squash,
            merging: false,
            pending_merge: Arc::new(Mutex::new(None)),
            walkthrough_state: walkthrough::WalkthroughState::Idle,
            #[cfg(not(target_arch = "wasm32"))]
            walkthrough_receiver: None,
            #[cfg(not(target_arch = "wasm32"))]
            walkthrough_response_text: String::new(),
            description_collapsed: true,
            conv_description_collapsed: false,
            collapsed_dirs: rustc_hash::FxHashSet::default(),
            file_tree_scroll_to_selected: false,
            file_panel_collapsed: false,
            seen_comment_ids: rustc_hash::FxHashSet::default(),
            expanded_comment_files: rustc_hash::FxHashSet::default(),
            resolved_thread_lines: rustc_hash::FxHashSet::default(),
            thread_node_ids: FxHashMap::default(),
            show_only_unresolved: false,
            pending_resolve: Arc::new(Mutex::new(None)),
            reviewed_files: rustc_hash::FxHashSet::default(),
            markdown_preview: false,
            markdown_scroll_y: 0.0,
            markdown_content_cache: None,
            file_opener: FileOpenerPopup::new(),
            repo_root: None,
            pending_open_file_opener: false,
            filter_active: false,
            filter_query: String::new(),
            pending_open_pr: None,
            pending_refresh: false,
            pending_go_back: false,
            bracket_pending: None,
            focused: false,
            preload_cache: FxHashMap::default(),
            pending_preloads: Vec::new(),
            preload_started: false,
            avatar_textures: FxHashMap::default(),
            avatar_fetched: rustc_hash::FxHashSet::default(),
            pending_avatars: Arc::new(Mutex::new(Vec::new())),
            http_client: reqwest::Client::new(),
            async_runtime,
            token: None,
            token_stable_frames: 0,
        }
    }

    /// Set the GitHub access token. Called each frame from workspace.
    pub fn set_token(&mut self, token: Option<String>) {
        let token_changed = token != self.token;
        if token_changed {
            // Token changed — reset the stability counter so we don't fire a
            // request with a token that's about to be replaced (e.g., OAuth
            // arriving first, then git credential replacing it).
            self.token_stable_frames = 0;
            // If we already have an error from a previous token, clear it so
            // the auto-fetch can retry with the new one.
            if token.is_some() && self.list_error.is_some() {
                self.list_error = None;
            }
            // Reset user login when token changes — we'll re-fetch.
            self.current_user_login = None;
        } else if self.token_stable_frames < 2 {
            self.token_stable_frames += 1;
            // Token just became stable (frame 2) — fetch the authenticated user
            // so the inbox segments can show "My PRs" and "Needs my review".
            if self.token_stable_frames == 2 && token.is_some() {
                self.fetch_authenticated_user();
            }
        }
        self.token = token;
    }

    /// Set the current user login directly (e.g. from workspace settings).
    pub fn set_current_user_login(&mut self, login: Option<String>) {
        self.current_user_login = login;
    }

    /// Fetch the authenticated GitHub user in the background.
    fn fetch_authenticated_user(&mut self) {
        let Some(token) = &self.token else { return };
        let client = self.http_client.clone();
        let token = token.clone();
        let pending = Arc::clone(&self.pending_user);
        self.async_runtime.spawn(async move {
            let result = api::get_authenticated_user(&client, &token).await;
            *pending.lock() = Some(result);
        });
    }

    /// Set the AI model ID from user settings.
    pub fn set_ai_model(&mut self, model: Option<String>) {
        self.ai_model = model;
    }

    /// Set whether this pane is the focused tile. Called each frame from workspace.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Close diff search if it is currently active.
    ///
    /// Returns `true` when search was active and got closed.
    pub(crate) fn close_diff_search_if_active(&mut self) -> bool {
        if self.diff_renderer.search_active() {
            self.diff_renderer.close_search();
            true
        } else {
            false
        }
    }

    /// Whether PR pane-owned text input mode is currently active.
    pub(crate) fn has_text_input_active(&self) -> bool {
        self.filter_active
            || self.commenting_line.is_some()
            || self.diff_renderer.search_active()
            || self.submit_panel_open
    }

    /// Set the repository root path for file opener. Called each frame from workspace.
    pub fn set_repo_root(&mut self, repo_root: Option<std::path::PathBuf>) {
        self.repo_root = repo_root;
    }

    /// Get the current PR number if one is open.
    pub fn current_pr_number(&self) -> Option<u32> {
        self.current_pr.as_ref().map(|pr| pr.number)
    }

    /// Get the list of changed file paths for the current PR.
    pub fn changed_file_paths(&self) -> Vec<String> {
        self.pr_files.iter().map(|f| f.filename.clone()).collect()
    }

    /// Set the review body text (used by AI agent commands).
    pub fn set_draft_body(&mut self, body: String) {
        self.draft_body = body;
    }

    /// Add a draft comment (used by AI agent commands).
    pub fn add_draft_comment(&mut self, path: String, line: usize, body: String) {
        self.draft_comments.push(DraftComment {
            path,
            line,
            side: "RIGHT".to_string(),
            body,
        });
    }

    // ── Test helpers ──

    /// Whether a review success message is displayed.
    #[doc(hidden)]
    pub fn has_submit_success(&self) -> bool {
        self.submit_success.is_some()
    }

    /// Whether a review error message is displayed.
    #[doc(hidden)]
    pub fn has_submit_error(&self) -> bool {
        self.submit_error.is_some()
    }

    /// Whether a comment is being drafted on a specific line.
    #[doc(hidden)]
    pub fn has_commenting_line(&self) -> bool {
        self.commenting_line.is_some()
    }

    /// Whether the submit review panel is open.
    #[doc(hidden)]
    pub fn is_submit_panel_open(&self) -> bool {
        self.submit_panel_open
    }

    /// Inject review state for testing. Simulates a completed review submission.
    #[doc(hidden)]
    pub fn simulate_submitted_review(&mut self) {
        self.submit_success = Some("Review submitted successfully".to_string());
        self.draft_body = "test body".to_string();
        self.commenting_line = Some((0, 10));
        self.comment_input = "in-progress comment".to_string();
        self.collapsed_threads.insert(("file.rs".to_string(), 5));
        self.submit_panel_open = true;
    }
}

// ── Session persistence (native only) ──

/// Persisted review session for a single PR.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[cfg(not(target_arch = "wasm32"))]
struct PrSession {
    reviewed_files: Vec<String>,
    seen_comment_ids: Vec<u64>,
    resolved_thread_lines: Vec<(String, usize)>,
    collapsed_dirs: Vec<String>,
    expanded_comment_files: Vec<String>,
    show_only_unresolved: bool,
    file_panel_collapsed: bool,
    description_collapsed: bool,
    markdown_preview: bool,
    selected_file_index: usize,
}

impl PrReviewPane {
    /// Build a `PrSession` snapshot from the current pane state.
    #[cfg(not(target_arch = "wasm32"))]
    fn build_session(&self) -> PrSession {
        PrSession {
            reviewed_files: self.reviewed_files.iter().cloned().collect(),
            seen_comment_ids: self.seen_comment_ids.iter().copied().collect(),
            resolved_thread_lines: self.resolved_thread_lines.iter().cloned().collect(),
            collapsed_dirs: self.collapsed_dirs.iter().cloned().collect(),
            expanded_comment_files: self.expanded_comment_files.iter().cloned().collect(),
            show_only_unresolved: self.show_only_unresolved,
            file_panel_collapsed: self.file_panel_collapsed,
            description_collapsed: self.description_collapsed,
            markdown_preview: self.markdown_preview,
            selected_file_index: self.selected_file_index,
        }
    }

    /// Apply a loaded `PrSession` into the current pane state.
    #[cfg(not(target_arch = "wasm32"))]
    fn apply_session(&mut self, session: PrSession) {
        self.reviewed_files = session.reviewed_files.into_iter().collect();
        self.seen_comment_ids = session.seen_comment_ids.into_iter().collect();
        self.resolved_thread_lines = session.resolved_thread_lines.into_iter().collect();
        self.collapsed_dirs = session.collapsed_dirs.into_iter().collect();
        self.expanded_comment_files = session.expanded_comment_files.into_iter().collect();
        self.show_only_unresolved = session.show_only_unresolved;
        self.file_panel_collapsed = session.file_panel_collapsed;
        self.description_collapsed = session.description_collapsed;
        self.markdown_preview = session.markdown_preview;
        self.selected_file_index = session
            .selected_file_index
            .min(self.file_diffs.len().saturating_sub(1));
    }

    /// Compute the filesystem path for a PR session file.
    #[cfg(not(target_arch = "wasm32"))]
    fn session_path(&self, pr_number: u32) -> Option<std::path::PathBuf> {
        let dir = enya_config::pr_sessions_dir();
        let filename = if let Some(ref user) = self.current_user_login {
            format!("{}_{}_{}_{}.json", self.owner, self.repo, pr_number, user)
        } else {
            format!("{}_{}_{}.json", self.owner, self.repo, pr_number)
        };
        Some(dir.join(filename))
    }

    /// Save the current review session to disk.
    #[cfg(not(target_arch = "wasm32"))]
    fn save_session(&self) {
        let Some(pr_number) = self.current_pr_number() else {
            return;
        };
        let Some(path) = self.session_path(pr_number) else {
            return;
        };
        let session = self.build_session();
        let json = match serde_json::to_string_pretty(&session) {
            Ok(j) => j,
            Err(e) => {
                log::warn!("Failed to serialize PR session: {e}");
                return;
            }
        };
        if let Err(e) = std::fs::write(&path, json) {
            log::warn!("Failed to write PR session: {e}");
        }
    }

    /// Load a previously saved review session from disk.
    #[cfg(not(target_arch = "wasm32"))]
    fn load_session(&mut self, pr_number: u32) {
        let Some(path) = self.session_path(pr_number) else {
            return;
        };
        let json = match std::fs::read_to_string(&path) {
            Ok(j) => j,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Failed to read PR session: {e}");
                }
                return;
            }
        };
        let session: PrSession = match serde_json::from_str(&json) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Failed to parse PR session: {e}");
                return;
            }
        };
        self.apply_session(session);
    }

    /// Delete the saved session for the current PR (called after submitting
    /// a review or when explicitly leaving the PR).
    #[cfg(not(target_arch = "wasm32"))]
    fn delete_session(&self) {
        let Some(pr_number) = self.current_pr_number() else {
            return;
        };
        let Some(path) = self.session_path(pr_number) else {
            return;
        };
        if let Err(e) = std::fs::remove_file(&path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Failed to delete PR session: {e}");
            }
        }
    }

    /// Rebuild the cached comment threads from `review_comments`.
    fn rebuild_thread_cache(&mut self) {
        self.cached_threads = crate::git::api::group_into_threads(&self.review_comments);
    }

    /// Merge GraphQL thread-resolution states into local state. Called after
    /// the GraphQL reviewThreads query completes.
    fn apply_thread_states(&mut self, states: Vec<api::ReviewThreadState>) {
        self.thread_node_ids.clear();
        // Keep any optimistic in-flight resolves — only overwrite where the
        // server gave us a concrete answer.
        for state in states {
            let key = (state.path.clone(), state.line);
            self.thread_node_ids.insert(key.clone(), state.thread_id);
            if state.is_resolved {
                self.resolved_thread_lines.insert(key);
            } else {
                self.resolved_thread_lines.remove(&key);
            }
        }
    }

    /// Mark all comments on the currently selected file as "seen".
    fn mark_current_file_comments_seen(&mut self) {
        let Some(file_diff) = self.file_diffs.get(self.selected_file_index) else {
            return;
        };
        let path = &file_diff.path;
        for comment in &self.review_comments {
            if comment.path.as_deref() == Some(path) {
                self.seen_comment_ids.insert(comment.id);
            }
        }
    }

    /// Toggle reviewed status for the currently selected file.
    /// When marking as reviewed (not un-marking), auto-advances to the next
    /// unreviewed file so the reviewer can fly through the file list.
    fn toggle_current_file_reviewed(&mut self) {
        let Some(file_diff) = self.file_diffs.get(self.selected_file_index) else {
            return;
        };
        let path = file_diff.path.clone();
        if self.reviewed_files.remove(&path) {
            // Un-marking — stay on the current file.
            return;
        }
        self.reviewed_files.insert(path);

        // Auto-advance: find the next unreviewed file after the current one,
        // wrapping around to the beginning if needed.
        let total = self.file_diffs.len();
        for offset in 1..total {
            let idx = (self.selected_file_index + offset) % total;
            if !self.reviewed_files.contains(&self.file_diffs[idx].path) {
                self.selected_file_index = idx;
                self.diff_renderer.reset_for_file_change();
                self.file_tree_scroll_to_selected = true;
                self.mark_current_file_comments_seen();
                self.markdown_preview = false;
                return;
            }
        }
        // All files reviewed — stay on current file.
    }

    /// Derive the aggregate review state for a PR from preloaded reviews.
    /// Returns `Some(Approved)` if at least one review is approved and none request changes,
    /// `Some(ChangesRequested)` if any review requests changes, or `None` otherwise.
    pub(super) fn review_state_for_pr(&self, number: u32) -> Option<ReviewState> {
        let reviews = self.preloaded_reviews.get(&number)?;
        // Build per-user latest state (only APPROVED / CHANGES_REQUESTED matter)
        let mut per_user: FxHashMap<&str, &str> = FxHashMap::default();
        for r in reviews {
            match r.state.as_str() {
                "APPROVED" | "CHANGES_REQUESTED" => {
                    per_user.insert(&r.user.login, &r.state);
                }
                _ => {}
            }
        }
        if per_user.is_empty() {
            return None;
        }
        if per_user.values().any(|s| *s == "CHANGES_REQUESTED") {
            return Some(ReviewState::ChangesRequested);
        }
        if per_user.values().any(|s| *s == "APPROVED") {
            return Some(ReviewState::Approved);
        }
        None
    }

    /// Post a single inline comment directly to the GitHub API (not batched into a review).
    pub(crate) fn post_single_comment(&mut self, path: String, line: usize, body: String) {
        let Some(token) = &self.token else { return };
        let Some(pr) = &self.current_pr else { return };

        self.submit_error = None;

        let client = self.http_client.clone();
        let token = token.clone();
        let owner = self.owner.clone();
        let repo = self.repo.clone();
        let number = pr.number;
        let commit_id = pr.head.sha.clone();
        let pending = Arc::clone(&self.pending_single_comment);

        self.async_runtime.spawn(async move {
            let result = api::create_review_comment(
                &client, &token, &owner, &repo, number, &commit_id, &path, line, &body,
            )
            .await;
            *pending.lock() = Some(result);
        });
    }

    /// Navigate to a specific comment thread: switch to its file and animate
    /// the diff scroll to the thread's line. Used by the file-tree thread entries.
    pub(super) fn navigate_to_thread(&mut self, thread_path: &str, thread_line: usize) {
        let Some(diff_idx) = self.file_diffs.iter().position(|d| d.path == thread_path) else {
            return;
        };
        if self.selected_file_index != diff_idx {
            self.selected_file_index = diff_idx;
            self.diff_renderer.reset_for_file_change();
            self.mark_current_file_comments_seen();
            self.markdown_preview = false;
            self.markdown_scroll_y = 0.0;
            self.markdown_content_cache = None;
            self.file_tree_scroll_to_selected = true;
        }
        let Some(file_diff) = self.file_diffs.get(self.selected_file_index) else {
            return;
        };
        // Threads are keyed by the new-side line number (GitHub's default for
        // review comments). Fall back to old-side for deletion-only threads.
        let target_line_idx = file_diff
            .lines
            .iter()
            .position(|l| l.new_line_num == Some(thread_line))
            .or_else(|| {
                file_diff
                    .lines
                    .iter()
                    .position(|l| l.old_line_num == Some(thread_line))
            });
        if let Some(idx) = target_line_idx {
            let target_y = idx as f32 * self.diff_renderer.line_height();
            self.diff_renderer.animate_scroll_to(target_y);
        }
    }

    /// Toggle thread resolution on a given (path, line).
    /// Updates local state immediately (optimistic) and kicks off the GraphQL
    /// mutation when a thread node id is known.
    pub(super) fn toggle_thread_resolved(&mut self, path: String, line: usize) {
        let key = (path, line);
        let currently_resolved = self.resolved_thread_lines.contains(&key);
        let new_resolved = !currently_resolved;
        if new_resolved {
            self.resolved_thread_lines.insert(key.clone());
        } else {
            self.resolved_thread_lines.remove(&key);
        }

        // If we have the GraphQL node id, fire the mutation.
        let Some(node_id) = self.thread_node_ids.get(&key).cloned() else {
            return;
        };
        let Some(token) = &self.token else { return };
        let client = self.http_client.clone();
        let token = token.clone();
        let pending = Arc::clone(&self.pending_resolve);
        self.async_runtime.spawn(async move {
            let result = api::set_thread_resolved(&client, &token, &node_id, new_resolved)
                .await
                .map(|_| (node_id, new_resolved));
            *pending.lock() = Some(result);
        });
    }

    /// Toggle resolution on the thread nearest to the current cursor/viewport line.
    pub(super) fn toggle_nearest_thread_resolved(&mut self) {
        let Some(file_diff) = self.file_diffs.get(self.selected_file_index) else {
            return;
        };
        let file_path = file_diff.path.clone();
        let file_threads: Vec<(usize, usize)> = self
            .cached_threads
            .iter()
            .filter(|t| t.path == file_path)
            .filter_map(|t| {
                file_diff
                    .lines
                    .iter()
                    .position(|l| l.new_line_num == Some(t.line))
                    .map(|line_idx| (line_idx, t.line))
            })
            .collect();
        if file_threads.is_empty() {
            return;
        }
        let current = self.diff_renderer.current_line_approx();
        let (_, target_line) = file_threads
            .iter()
            .min_by_key(|(line_idx, _)| (*line_idx as i64 - current as i64).abs())
            .copied()
            .unwrap_or(file_threads[0]);
        self.toggle_thread_resolved(file_path, target_line);
    }

    /// Jump to the next or previous comment thread in the current file's diff.
    fn jump_to_comment_thread(&mut self, dir: BracketDir) {
        let Some(file_diff) = self.file_diffs.get(self.selected_file_index) else {
            return;
        };

        // Use cached threads
        let threads = &self.cached_threads;
        let file_path = &file_diff.path;

        let mut comment_line_indices: Vec<usize> = Vec::new();
        for (line_idx, line) in file_diff.lines.iter().enumerate() {
            if let Some(new_line) = line.new_line_num {
                let has_thread = threads
                    .iter()
                    .any(|t| t.path == *file_path && t.line == new_line);
                let has_draft = self
                    .draft_comments
                    .iter()
                    .any(|c| c.path == *file_path && c.line == new_line);
                if has_thread || has_draft {
                    comment_line_indices.push(line_idx);
                }
            }
        }

        if comment_line_indices.is_empty() {
            return;
        }

        // Find the current scroll position and determine which thread to jump to
        let current_line = self.diff_renderer.current_line_approx();

        let target_idx = match dir {
            BracketDir::Next => comment_line_indices
                .iter()
                .find(|&&idx| idx > current_line + 2)
                .or(comment_line_indices.first()),
            BracketDir::Prev => comment_line_indices
                .iter()
                .rev()
                .find(|&&idx| idx + 2 < current_line)
                .or(comment_line_indices.last()),
        };

        if let Some(&target_line_idx) = target_idx {
            let target_y = target_line_idx as f32 * self.diff_renderer.line_height();
            self.diff_renderer.animate_scroll_to(target_y);
        }
    }

    /// Navigate to a specific PR by number. Uses preloaded data if available.
    /// Clear per-PR review and comment state (drafts, success/error messages, UI toggles).
    fn clear_review_state(&mut self) {
        self.submit_success = None;
        self.submit_error = None;
        self.commenting_line = None;
        self.comment_input.clear();
        self.draft_comments.clear();
        self.draft_body.clear();
        self.collapsed_threads.clear();
        self.reviewed_files.clear();
        self.floating_card_heights.clear();
        self.submit_panel_open = false;
        self.flash_start = None;
        self.auto_surface_dismissed = false;
        self.merge_popup_open = false;
        self.merging = false;
        self.walkthrough_state = walkthrough::WalkthroughState::Idle;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.walkthrough_receiver = None;
            self.walkthrough_response_text.clear();
        }
    }

    pub fn open_pr(&mut self, number: u32) {
        self.clear_review_state();

        // Check preload cache first
        if let Some(cached) = self.preload_cache.remove(&number) {
            self.view = PrReviewView::Detail;
            self.detail_loading = false;
            self.detail_error = None;
            self.current_pr = Some(cached.pr);
            self.pr_files = cached.files;
            self.file_diffs = cached.file_diffs;
            self.review_comments = cached.review_comments;
            self.issue_comments = cached.issue_comments;
            self.check_runs = cached.check_runs;
            self.reviews = cached.reviews.clone();
            self.preloaded_reviews.insert(number, cached.reviews);
            self.selected_file_index = 0;
            self.mark_current_file_comments_seen();
            #[cfg(not(target_arch = "wasm32"))]
            self.load_session(number);
            return;
        }

        if let Some(token) = &self.token {
            self.view = PrReviewView::Detail;
            self.detail_loading = true;
            self.detail_error = None;

            let client = self.http_client.clone();
            let token = token.clone();
            let owner = self.owner.clone();
            let repo = self.repo.clone();
            let pending = Arc::clone(&self.pending_detail);

            self.async_runtime.spawn(async move {
                let result = async {
                    let pr = api::get_pull(&client, &token, &owner, &repo, number).await?;
                    let files = api::get_pull_files(&client, &token, &owner, &repo, number).await?;
                    let diff =
                        api::get_pull_diff(&client, &token, &owner, &repo, number, Some(&files))
                            .await?;
                    Ok((pr, files, diff))
                }
                .await;
                *pending.lock() = Some(result);
            });

            // Also fetch comments and checks
            self.fetch_comments(number);
            self.fetch_checks(number);
        }
    }

    /// Fetch the list of open PRs.
    fn fetch_pr_list(&mut self) {
        let Some(token) = &self.token else {
            return;
        };

        self.list_loading = true;
        self.list_error = None;

        let client = self.http_client.clone();
        let token = token.clone();
        let owner = self.owner.clone();
        let repo = self.repo.clone();
        let pending = Arc::clone(&self.pending_list);

        self.async_runtime.spawn(async move {
            let result = api::list_open_pulls(&client, &token, &owner, &repo).await;
            *pending.lock() = Some(result);
        });
    }

    /// Fetch comments for a PR.
    fn fetch_comments(&mut self, number: u32) {
        let Some(token) = &self.token else {
            return;
        };

        let client = self.http_client.clone();
        let token = token.clone();
        let owner = self.owner.clone();
        let repo = self.repo.clone();
        let pending = Arc::clone(&self.pending_comments);

        self.async_runtime.spawn(async move {
            let result = async {
                let review_comments =
                    api::get_review_comments(&client, &token, &owner, &repo, number).await?;
                let issue_comments =
                    api::get_issue_comments(&client, &token, &owner, &repo, number).await?;
                let reviews = api::get_reviews(&client, &token, &owner, &repo, number).await?;
                // Thread resolution state is best-effort — if the GraphQL call
                // fails (e.g., scoped token without repo access), return an
                // empty list rather than failing the whole fetch.
                let thread_states =
                    api::get_review_thread_states(&client, &token, &owner, &repo, number)
                        .await
                        .unwrap_or_else(|err| {
                            log::warn!("review thread state fetch failed: {err}");
                            Vec::new()
                        });
                Ok((review_comments, issue_comments, reviews, thread_states))
            }
            .await;
            *pending.lock() = Some(result);
        });
    }

    /// Fetch check runs for a PR.
    fn fetch_checks(&mut self, _number: u32) {
        let Some(token) = &self.token else {
            return;
        };

        // We need the head SHA — if we have the PR already, use it
        let head_sha = self
            .current_pr
            .as_ref()
            .map(|pr| pr.head.sha.clone())
            .unwrap_or_default();

        if head_sha.is_empty() {
            // We'll fetch checks after the PR detail arrives
            return;
        }

        let client = self.http_client.clone();
        let token = token.clone();
        let owner = self.owner.clone();
        let repo = self.repo.clone();
        let pending = Arc::clone(&self.pending_checks);

        self.async_runtime.spawn(async move {
            let result = api::get_check_runs(&client, &token, &owner, &repo, &head_sha).await;
            *pending.lock() = Some(result);
        });
    }

    /// Kick off background fetches for the first N PRs in the list.
    fn start_preloading(&mut self) {
        let Some(token) = &self.token else {
            return;
        };

        self.preload_started = true;

        let prs_to_preload: Vec<_> = self
            .pull_requests
            .iter()
            .take(PRELOAD_COUNT)
            .filter(|pr| !self.preload_cache.contains_key(&pr.number))
            .map(|pr| pr.number)
            .collect();

        for number in prs_to_preload {
            let client = self.http_client.clone();
            let token = token.clone();
            let owner = self.owner.clone();
            let repo = self.repo.clone();
            let pending: PendingPreload = Arc::new(Mutex::new(None));
            let pending_clone = Arc::clone(&pending);
            self.pending_preloads.push(pending);

            self.async_runtime.spawn(async move {
                let result = async {
                    let pr = api::get_pull(&client, &token, &owner, &repo, number).await?;
                    let files = api::get_pull_files(&client, &token, &owner, &repo, number).await?;
                    let diff =
                        api::get_pull_diff(&client, &token, &owner, &repo, number, Some(&files))
                            .await?;
                    let mut file_diffs = crate::git::diff::parse_diff_into_files(&diff);
                    file_diffs.sort_by(|a, b| a.path.cmp(&b.path));
                    let review_comments =
                        api::get_review_comments(&client, &token, &owner, &repo, number).await?;
                    let issue_comments =
                        api::get_issue_comments(&client, &token, &owner, &repo, number).await?;
                    let reviews = api::get_reviews(&client, &token, &owner, &repo, number).await?;
                    let check_runs = if !pr.head.sha.is_empty() {
                        api::get_check_runs(&client, &token, &owner, &repo, &pr.head.sha)
                            .await
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    Ok(PreloadedPr {
                        pr,
                        files,
                        file_diffs,
                        review_comments,
                        issue_comments,
                        check_runs,
                        reviews,
                    })
                }
                .await;
                *pending_clone.lock() = Some((number, result));
            });
        }
    }

    /// Submit the current review draft.
    pub(crate) fn submit_review(&mut self, event: ReviewEvent) {
        let Some(token) = &self.token else {
            return;
        };
        let Some(pr) = &self.current_pr else {
            return;
        };

        self.submitting_review = true;
        self.submit_error = None;
        self.submit_success = None;

        let client = self.http_client.clone();
        let token = token.clone();
        let owner = self.owner.clone();
        let repo = self.repo.clone();
        let number = pr.number;
        let body = if self.draft_body.is_empty() {
            None
        } else {
            Some(self.draft_body.clone())
        };
        let comments = self.draft_comments.clone();
        let pending = Arc::clone(&self.pending_submit);

        self.async_runtime.spawn(async move {
            let result = api::submit_review(
                &client, &token, &owner, &repo, number, event, body, comments,
            )
            .await;
            *pending.lock() = Some(result);
        });
    }

    /// Merge the current pull request.
    pub(crate) fn merge_pull(&mut self) {
        let Some(token) = &self.token else {
            return;
        };
        let Some(pr) = &self.current_pr else {
            return;
        };

        self.merging = true;
        self.submit_error = None;
        self.submit_success = None;

        let client = self.http_client.clone();
        let token = token.clone();
        let owner = self.owner.clone();
        let repo = self.repo.clone();
        let number = pr.number;
        let node_id = pr.node_id.clone();
        let merge_method = self.merge_method;
        let pending = Arc::clone(&self.pending_merge);

        self.async_runtime.spawn(async move {
            let result = api::merge_pull(
                &client,
                &token,
                &owner,
                &repo,
                number,
                &node_id,
                None,
                merge_method,
            )
            .await;
            *pending.lock() = Some(result);
        });
    }

    /// Request an AI-powered review walkthrough for the current PR.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn request_walkthrough(&mut self) {
        let Some(pr) = &self.current_pr else {
            return;
        };
        if matches!(
            self.walkthrough_state,
            walkthrough::WalkthroughState::Loading
        ) {
            return; // Already in progress
        }

        let prompt =
            walkthrough::build_walkthrough_prompt(&pr.title, pr.body.as_deref(), &self.file_diffs);

        self.walkthrough_state = walkthrough::WalkthroughState::Loading;
        self.walkthrough_response_text.clear();

        let receiver = walkthrough::spawn_walkthrough_request(
            &self.async_runtime,
            prompt,
            self.ai_model.as_deref(),
        );
        self.walkthrough_receiver = Some(receiver);
    }

    /// WASM stub — walkthrough not available in browser.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn request_walkthrough(&mut self) {
        self.walkthrough_state = walkthrough::WalkthroughState::Error(
            "AI walkthrough is not available in the browser".to_string(),
        );
    }

    /// Poll for async operation results. Called each frame.
    fn poll_results(&mut self, ctx: &egui::Context) {
        // Poll authenticated user fetch
        let user_result = self.pending_user.lock().take();
        if let Some(result) = user_result {
            match result {
                Ok(user) => self.current_user_login = Some(user.login),
                Err(e) => log::warn!("Failed to fetch authenticated user: {e}"),
            }
        }

        // Poll PR list (extract result before borrowing self mutably)
        let list_result = self.pending_list.lock().take();
        if let Some(result) = list_result {
            self.list_loading = false;
            match result {
                Ok(prs) => {
                    // Preserve selection by PR number across refresh
                    let selected_number = self
                        .pull_requests
                        .get(self.selected_pr_index)
                        .map(|pr| pr.number);
                    self.pull_requests = prs;
                    if let Some(number) = selected_number {
                        self.selected_pr_index = self
                            .pull_requests
                            .iter()
                            .position(|pr| pr.number == number)
                            .unwrap_or(0);
                    }
                    self.list_error = None;
                    // Fetch avatars for PR authors
                    self.fetch_avatars_for_pr_list();
                    // Kick off preloading for the top PRs
                    self.preload_started = false;
                }
                Err(e) => {
                    self.list_error = Some(e);
                }
            }
        }

        // Start preloading if we have PRs and haven't started yet
        if !self.preload_started && !self.pull_requests.is_empty() && self.token.is_some() {
            self.start_preloading();
        }

        // Poll preload results
        self.pending_preloads.retain(|pending| {
            let mut guard = pending.lock();
            if let Some((number, result)) = guard.take() {
                match result {
                    Ok(preloaded) => {
                        // Compute merge-readiness before caching
                        let is_approved = {
                            let mut per_user: rustc_hash::FxHashMap<&str, &str> =
                                rustc_hash::FxHashMap::default();
                            for r in &preloaded.reviews {
                                if r.state == "APPROVED" || r.state == "CHANGES_REQUESTED" {
                                    per_user.insert(&r.user.login, &r.state);
                                }
                            }
                            !per_user.is_empty() && per_user.values().all(|s| *s == "APPROVED")
                        };
                        let all_checks_pass = !preloaded.check_runs.is_empty()
                            && preloaded.check_runs.iter().all(|c| {
                                matches!(c.conclusion.as_deref(), Some("success") | Some("skipped"))
                            });
                        let mergeable = preloaded.pr.mergeable.unwrap_or(false);
                        let merge_ready =
                            is_approved && all_checks_pass && mergeable && !preloaded.pr.draft;
                        self.preloaded_merge_ready.insert(number, merge_ready);

                        // Comment count
                        // Comment count
                        let comment_count =
                            preloaded.review_comments.len() + preloaded.issue_comments.len();
                        self.preloaded_comment_counts.insert(number, comment_count);

                        self.preloaded_reviews
                            .insert(number, preloaded.reviews.clone());
                        self.preload_cache.insert(number, preloaded);
                    }
                    Err(e) => {
                        log::warn!("Failed to preload PR #{number}: {e}");
                    }
                }
                false // remove from pending list
            } else {
                true // still in flight
            }
        });

        // Poll PR detail
        let detail_result = self.pending_detail.lock().take();
        if let Some(result) = detail_result {
            self.detail_loading = false;
            match result {
                Ok((pr, files, diff)) => {
                    let mut file_diffs = crate::git::diff::parse_diff_into_files(&diff);
                    file_diffs.sort_by(|a, b| a.path.cmp(&b.path));
                    self.file_diffs = file_diffs;
                    let pr_number = pr.number;
                    let head_sha_empty = pr.head.sha.is_empty();
                    self.current_pr = Some(pr);
                    self.pr_files = files;
                    self.selected_file_index = 0;
                    self.detail_error = None;
                    self.mark_current_file_comments_seen();
                    #[cfg(not(target_arch = "wasm32"))]
                    self.load_session(pr_number);

                    // Now fetch checks with the head SHA
                    if !head_sha_empty {
                        self.fetch_checks(pr_number);
                    }
                }
                Err(e) => {
                    self.detail_error = Some(e);
                }
            }
        }

        // Poll comments
        let comments_result = self.pending_comments.lock().take();
        if let Some(result) = comments_result {
            match result {
                Ok((review_comments, issue_comments, reviews, thread_states)) => {
                    self.review_comments = review_comments;
                    self.rebuild_thread_cache();
                    self.issue_comments = issue_comments;
                    self.reviews = reviews.clone();
                    if let Some(pr) = &self.current_pr {
                        self.preloaded_reviews.insert(pr.number, reviews);
                    }
                    self.apply_thread_states(thread_states);
                    self.fetch_avatars_for_comments();
                }
                Err(e) => {
                    log::warn!("Failed to fetch PR comments: {e}");
                }
            }
        }

        // Poll thread resolve/unresolve result
        if let Some(result) = self.pending_resolve.lock().take() {
            match result {
                Ok((node_id, new_resolved)) => {
                    // Find the (path, line) key for this node_id and ensure
                    // our local state matches the server response.
                    let key = self
                        .thread_node_ids
                        .iter()
                        .find(|(_, id)| *id == &node_id)
                        .map(|(k, _)| k.clone());
                    if let Some(key) = key {
                        if new_resolved {
                            self.resolved_thread_lines.insert(key);
                        } else {
                            self.resolved_thread_lines.remove(&key);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to toggle thread resolution: {e}");
                }
            }
        }

        // Poll checks
        if let Some(result) = self.pending_checks.lock().take() {
            match result {
                Ok(checks) => {
                    self.check_runs = checks;
                }
                Err(e) => {
                    log::warn!("Failed to fetch check runs: {e}");
                }
            }
        }

        // Poll review submission
        let submit_result = self.pending_submit.lock().take();
        if let Some(result) = submit_result {
            self.submitting_review = false;
            match result {
                Ok(()) => {
                    self.submit_success = Some("Review submitted successfully".to_string());
                    self.flash_start = Some(crate::util::Instant::now());
                    self.flash_is_success = true;
                    self.submit_panel_open = false;
                    self.draft_comments.clear();
                    self.draft_body.clear();
                    // Review is complete — clear the persisted session
                    #[cfg(not(target_arch = "wasm32"))]
                    self.delete_session();
                    // Refresh comments
                    let pr_number = self.current_pr.as_ref().map(|pr| pr.number);
                    if let Some(number) = pr_number {
                        self.fetch_comments(number);
                    }
                }
                Err(e) => {
                    self.submit_error = Some(e);
                    self.flash_start = Some(crate::util::Instant::now());
                    self.flash_is_success = false;
                }
            }
        }

        // Poll merge result
        let merge_result = self.pending_merge.lock().take();
        if let Some(result) = merge_result {
            self.merging = false;
            match result {
                Ok(outcome) => {
                    let msg = match outcome {
                        api::MergeOutcome::Merged => "Pull request merged",
                        api::MergeOutcome::AutoMergeEnabled => "Auto-merge enabled",
                    };
                    self.submit_success = Some(msg.to_string());
                    self.flash_start = Some(crate::util::Instant::now());
                    self.flash_is_success = true;
                    self.merge_popup_open = false;
                    // PR is closed — clear the persisted session
                    #[cfg(not(target_arch = "wasm32"))]
                    self.delete_session();
                    // Refresh the PR list to reflect the merged state
                    self.fetch_pr_list();
                }
                Err(e) => {
                    self.submit_error = Some(e);
                }
            }
        }

        // Poll single comment post
        let single_comment_result = self.pending_single_comment.lock().take();
        if let Some(result) = single_comment_result {
            match result {
                Ok(comment) => {
                    // Append the new comment and rebuild thread cache
                    self.review_comments.push(comment);
                    self.rebuild_thread_cache();
                    self.fetch_avatars_for_comments();
                }
                Err(e) => {
                    self.submit_error = Some(e);
                }
            }
        }

        // Poll walkthrough streaming events (native only — ACP not available on WASM)
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(ref receiver) = self.walkthrough_receiver {
            let mut done = false;
            while let Ok(event) = receiver.try_recv() {
                match event {
                    enya_ai::AgentEvent::TextDelta(text) => {
                        self.walkthrough_response_text.push_str(&text);
                    }
                    enya_ai::AgentEvent::Done { .. } => {
                        done = true;
                        break;
                    }
                    enya_ai::AgentEvent::Error(e) => {
                        self.walkthrough_state =
                            walkthrough::WalkthroughState::Error(format!("AI error: {e}"));
                        self.walkthrough_receiver = None;
                        done = false;
                        break;
                    }
                    _ => {}
                }
            }
            if done {
                let result =
                    walkthrough::parse_walkthrough_response(&self.walkthrough_response_text);
                match result {
                    Ok(wt) => {
                        self.walkthrough_state = walkthrough::WalkthroughState::Ready(wt);
                    }
                    Err(e) => {
                        self.walkthrough_state = walkthrough::WalkthroughState::Error(e);
                    }
                }
                self.walkthrough_receiver = None;
            }
        }

        // Poll avatar fetches
        let avatar_results: Vec<_> = self.pending_avatars.lock().drain(..).collect();
        for (login, result) in avatar_results {
            if let Ok(bytes) = result {
                if let Ok(dynamic_image) = image::load_from_memory(&bytes) {
                    let rgba = dynamic_image.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let pixels: Vec<egui::Color32> = rgba
                        .pixels()
                        .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
                        .collect();
                    let color_image = egui::ColorImage::new(size, pixels);
                    let texture = ctx.load_texture(
                        format!("avatar_{login}"),
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.avatar_textures.insert(login, texture);
                }
            }
        }
    }

    /// Kick off avatar fetches for all unique users in the current comments.
    fn fetch_avatars_for_comments(&mut self) {
        let mut users: Vec<(String, String)> = Vec::new(); // (login, avatar_url)

        // Collect from review comments
        for comment in &self.review_comments {
            if !comment.user.avatar_url.is_empty()
                && !self.avatar_fetched.contains(&comment.user.login)
            {
                self.avatar_fetched.insert(comment.user.login.clone());
                users.push((comment.user.login.clone(), comment.user.avatar_url.clone()));
            }
        }
        // Collect from issue comments
        for comment in &self.issue_comments {
            if !comment.user.avatar_url.is_empty()
                && !self.avatar_fetched.contains(&comment.user.login)
            {
                self.avatar_fetched.insert(comment.user.login.clone());
                users.push((comment.user.login.clone(), comment.user.avatar_url.clone()));
            }
        }
        // Collect from PR author
        if let Some(pr) = &self.current_pr {
            if !pr.user.avatar_url.is_empty() && !self.avatar_fetched.contains(&pr.user.login) {
                self.avatar_fetched.insert(pr.user.login.clone());
                users.push((pr.user.login.clone(), pr.user.avatar_url.clone()));
            }
        }

        if users.is_empty() {
            return;
        }

        let client = self.http_client.clone();
        let pending = Arc::clone(&self.pending_avatars);

        self.async_runtime.spawn(async move {
            for (login, avatar_url) in users {
                let result = crate::git::auth::fetch_avatar(&client, &avatar_url).await;
                pending.lock().push((login, result));
            }
        });
    }

    /// Kick off avatar fetches for all unique PR authors in the list view.
    fn fetch_avatars_for_pr_list(&mut self) {
        let mut users: Vec<(String, String)> = Vec::new();

        for pr in &self.pull_requests {
            if !pr.user.avatar_url.is_empty() && !self.avatar_fetched.contains(&pr.user.login) {
                self.avatar_fetched.insert(pr.user.login.clone());
                users.push((pr.user.login.clone(), pr.user.avatar_url.clone()));
            }
        }

        if users.is_empty() {
            return;
        }

        let client = self.http_client.clone();
        let pending = Arc::clone(&self.pending_avatars);

        self.async_runtime.spawn(async move {
            for (login, avatar_url) in users {
                let result = crate::git::auth::fetch_avatar(&client, &avatar_url).await;
                pending.lock().push((login, result));
            }
        });
    }
}

// ── Component trait implementation ───────────────────────────────────────

impl PrReviewPane {
    /// Handle vim-style keyboard navigation. Call before rendering.
    ///
    /// Mostly consumes keys only when the pane is the focused tile. In list
    /// view, Escape is not consumed so the workspace can unfocus the pane. In
    /// detail view, Escape/h go back to list first. Exception: when diff
    /// search is active, Escape can still close it even if tile focus was lost.
    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        // When a text input we own is active, handle only Escape/Enter/Cmd+Enter
        // and skip normal vim keys. We check our own state rather than
        // ctx.memory(focused) which is too broad (spinners, buttons, etc.).
        let has_text_input = self.filter_active
            || self.commenting_line.is_some()
            || self.diff_renderer.search_active();
        if has_text_input {
            // Allow Escape to close diff search even when tile focus was lost.
            // Keep list filter/comment handling scoped to the focused pane.
            if !self.focused && !self.diff_renderer.search_active() {
                return;
            }

            let esc = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if esc {
                if self.filter_active {
                    self.filter_active = false;
                    self.filter_query.clear();
                    self.selected_pr_index = 0;
                } else if self.diff_renderer.search_active() {
                    self.diff_renderer.close_search();
                } else if self.commenting_line.is_some() {
                    self.commenting_line = None;
                    self.comment_input.clear();
                }
                if self.focused {
                    if let Some(id) = ctx.memory(|m| m.focused()) {
                        ctx.memory_mut(|mem| mem.surrender_focus(id));
                    }
                }
            }
            // Cmd+Enter (or Ctrl+Enter) — submit comment while typing
            if self.commenting_line.is_some() {
                let cmd_enter =
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
                if cmd_enter && !self.comment_input.is_empty() {
                    if let Some((file_idx, line_idx)) = self.commenting_line {
                        // Resolve the new_line_num for this diff line and post directly
                        if let Some(file_diff) = self.file_diffs.get(file_idx) {
                            if let Some(line) = file_diff.lines.get(line_idx) {
                                if let Some(new_line) = line.new_line_num {
                                    let path = file_diff.path.clone();
                                    let body = self.comment_input.clone();
                                    self.post_single_comment(path, new_line, body);
                                }
                            }
                        }
                        self.comment_input.clear();
                        self.commenting_line = None;
                    }
                }
            }
            // Enter in filter mode closes the bar but keeps the query
            if self.filter_active {
                let enter =
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                if enter {
                    self.filter_active = false;
                    if let Some(id) = ctx.memory(|m| m.focused()) {
                        ctx.memory_mut(|mem| mem.surrender_focus(id));
                    }
                }
            }
            return;
        }

        if !self.focused {
            return;
        }

        // If a widget has focus (e.g. command palette, finder, comment input
        // in another pane), don't steal its key events.  The one exception is
        // the viewport filter TextEdit in the toolbar — it's always rendered
        // and can grab stale focus, so we clear it and proceed.
        let viewport_filter_id = egui::Id::new("viewport_filter_inline");
        if let Some(focused) = ctx.memory(|m| m.focused()) {
            if focused == viewport_filter_id {
                ctx.memory_mut(|mem| mem.surrender_focus(viewport_filter_id));
            } else {
                // Some other text input (finder, etc.) has focus — don't consume
                // keys, let the overlay handle them.
                return;
            }
        }

        ctx.input_mut(|input| {
            match self.view {
                PrReviewView::List => {
                    // In list view: j/k navigate, Enter/l open PR.
                    // Escape, h, and x are NOT consumed — they pass through to the
                    // workspace so it can unfocus, navigate, or close the pane.
                    let filtered = self.filtered_pr_indices();
                    let count = filtered.len();

                    // Always consume navigation keys to prevent workspace from
                    // stealing them (even when list is empty/loading).
                    let down = input.consume_key(egui::Modifiers::NONE, egui::Key::J)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
                    let up = input.consume_key(egui::Modifiers::NONE, egui::Key::K)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
                    let open = input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::L)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight);
                    let refresh = input.consume_key(egui::Modifiers::NONE, egui::Key::R);
                    let jump_bottom = input.consume_key(egui::Modifiers::SHIFT, egui::Key::G);
                    let jump_top = input.consume_key(egui::Modifiers::NONE, egui::Key::G);

                    // Only act on navigation when we have PRs to navigate
                    if count > 0 {
                        if down {
                            self.selected_pr_index =
                                (self.selected_pr_index + 1).min(count.saturating_sub(1));
                            self.list_scroll_to_selected = true;
                        }
                        if up {
                            self.selected_pr_index = self.selected_pr_index.saturating_sub(1);
                            self.list_scroll_to_selected = true;
                        }
                        if open {
                            if let Some(&pr_idx) = filtered.get(self.selected_pr_index) {
                                if let Some(pr) = self.pull_requests.get(pr_idx) {
                                    self.pending_open_pr = Some(pr.number);
                                }
                            }
                        }
                        if jump_bottom {
                            self.selected_pr_index = count.saturating_sub(1);
                            self.list_scroll_to_selected = true;
                        }
                        if jump_top {
                            self.selected_pr_index = 0;
                            self.list_scroll_to_selected = true;
                        }
                    }
                    if refresh {
                        self.pending_refresh = true;
                    }

                    // 1/2/3 — switch inbox segment
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::Num1) {
                        self.list_segment = PrListSegment::NeedsReview;
                        self.selected_pr_index = 0;
                    }
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::Num2) {
                        self.list_segment = PrListSegment::MyPrs;
                        self.selected_pr_index = 0;
                    }
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::Num3) {
                        self.list_segment = PrListSegment::All;
                        self.selected_pr_index = 0;
                    }

                    // / — activate filter (consume both Key::Slash and Text("/")
                    // so the workspace viewport filter doesn't steal the event)
                    let slash_key = input.consume_key(egui::Modifiers::NONE, egui::Key::Slash);
                    let slash_text = input
                        .events
                        .iter()
                        .any(|e| matches!(e, egui::Event::Text(t) if t == "/"));
                    if slash_key || slash_text {
                        input
                            .events
                            .retain(|e| !matches!(e, egui::Event::Text(t) if t == "/"));
                        self.filter_active = true;
                        self.filter_query.clear();
                        self.selected_pr_index = 0;
                    }

                    // Escape — close filter if active (fallback for when TextEdit
                    // steals focus before handle_keyboard can consume Escape)
                    if self.filter_active
                        && input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    {
                        self.filter_active = false;
                        self.filter_query.clear();
                        self.selected_pr_index = 0;
                    }
                }
                PrReviewView::Detail => {
                    // Consume x/u in detail view to prevent accidental pane close
                    input.consume_key(egui::Modifiers::NONE, egui::Key::X);
                    input.consume_key(egui::Modifiers::NONE, egui::Key::U);

                    // v — toggle current file as reviewed ("viewed")
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::V) {
                        self.toggle_current_file_reviewed();
                    }

                    // Shift+R — toggle resolution on the thread nearest the cursor
                    if input.consume_key(egui::Modifiers::SHIFT, egui::Key::R) {
                        self.toggle_nearest_thread_resolved();
                    }

                    // In detail view: Escape closes search first, then goes back.
                    // h/Backspace/ArrowLeft always go back.
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                        if self.diff_renderer.search_active() {
                            self.diff_renderer.close_search();
                        } else {
                            self.pending_go_back = true;
                        }
                    }
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::H)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
                    {
                        self.pending_go_back = true;
                    }

                    // When markdown preview is active, handle scroll keys directly
                    // instead of delegating to the diff renderer.
                    if self.markdown_preview {
                        let scroll_step = 40.0;
                        if input.consume_key(egui::Modifiers::NONE, egui::Key::J) {
                            self.markdown_scroll_y += scroll_step;
                        }
                        if input.consume_key(egui::Modifiers::NONE, egui::Key::K) {
                            self.markdown_scroll_y =
                                (self.markdown_scroll_y - scroll_step).max(0.0);
                        }
                        if input.consume_key(egui::Modifiers::NONE, egui::Key::D)
                            && input.modifiers.ctrl
                        {
                            self.markdown_scroll_y += 300.0;
                        }
                        if input.consume_key(egui::Modifiers::NONE, egui::Key::U)
                            && input.modifiers.ctrl
                        {
                            self.markdown_scroll_y = (self.markdown_scroll_y - 300.0).max(0.0);
                        }
                        if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                            self.markdown_scroll_y += 60.0;
                        }
                        if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                            self.markdown_scroll_y = (self.markdown_scroll_y - 60.0).max(0.0);
                        }
                        // G = jump to bottom (large offset), gg = jump to top
                        if input.consume_key(egui::Modifiers::SHIFT, egui::Key::G) {
                            self.markdown_scroll_y = f32::MAX;
                        }
                    }

                    // Delegate standard diff keys to renderer
                    let action = self.diff_renderer.handle_keyboard(input);
                    match action {
                        DiffKeyAction::NextFile => {
                            let max = self.file_diffs.len().saturating_sub(1);
                            self.selected_file_index = (self.selected_file_index + 1).min(max);
                            self.diff_renderer.reset_for_file_change();
                            self.file_tree_scroll_to_selected = true;
                            self.mark_current_file_comments_seen();
                            self.markdown_preview = false;
                            self.markdown_scroll_y = 0.0;
                            self.markdown_content_cache = None;
                        }
                        DiffKeyAction::PrevFile => {
                            self.selected_file_index = self.selected_file_index.saturating_sub(1);
                            self.diff_renderer.reset_for_file_change();
                            self.file_tree_scroll_to_selected = true;
                            self.mark_current_file_comments_seen();
                            self.markdown_preview = false;
                            self.markdown_scroll_y = 0.0;
                            self.markdown_content_cache = None;
                        }
                        DiffKeyAction::OpenFile => {
                            self.pending_open_file_opener = true;
                        }
                        DiffKeyAction::CopySelected => {
                            // Copy selected text if any
                        }
                        DiffKeyAction::CommentOnSelected => {
                            // Open comment input on the selected line, or the line
                            // at the center of the viewport if nothing is selected
                            let idx = if let Some((start, _)) = self.diff_renderer.selected_lines()
                            {
                                start
                            } else {
                                self.diff_renderer.current_line_approx()
                            };
                            self.commenting_line = Some((self.selected_file_index, idx));
                        }
                        DiffKeyAction::None => {}
                    }

                    // Arrow keys for diff scrolling (supplement vim keys)
                    let scroll_amount = 60.0;
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                        self.diff_renderer.scroll_down(scroll_amount);
                    }
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                        self.diff_renderer.scroll_up(scroll_amount);
                    }

                    // ] / [ — bracket prefix for two-key sequences
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::CloseBracket)
                        && !input.modifiers.shift
                    {
                        self.bracket_pending = Some(BracketDir::Next);
                    }
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::OpenBracket)
                        && !input.modifiers.shift
                    {
                        self.bracket_pending = Some(BracketDir::Prev);
                    }

                    // ]c / [c — jump to next/prev comment thread
                    if let Some(dir) = self.bracket_pending {
                        if input.consume_key(egui::Modifiers::NONE, egui::Key::C) {
                            self.bracket_pending = None;
                            self.jump_to_comment_thread(dir);
                        } else if input.events.iter().any(|e| {
                            matches!(
                                e,
                                egui::Event::Key {
                                    pressed: true,
                                    key,
                                    ..
                                } if *key != egui::Key::OpenBracket
                                    && *key != egui::Key::CloseBracket
                            )
                        }) {
                            // Any other key cancels the bracket prefix
                            self.bracket_pending = None;
                        }
                    }

                    // 1/2/3 — switch tabs
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::Num1) {
                        self.active_tab = DetailTab::Files;
                    }
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::Num2) {
                        self.active_tab = DetailTab::Conversation;
                    }
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::Num3) {
                        self.active_tab = DetailTab::Checks;
                    }
                }
            }
        });
    }
}

impl crate::components::Component for PrReviewPane {
    fn show(&mut self, ui: &mut egui::Ui) {
        // Poll async results
        self.poll_results(ui.ctx());

        // Handle keyboard navigation
        self.handle_keyboard(ui.ctx());

        // Process deferred keyboard actions (need &mut self outside input closure)
        if let Some(number) = self.pending_open_pr.take() {
            self.open_pr(number);
        }
        if self.pending_refresh {
            self.pending_refresh = false;
            self.fetch_pr_list();
        }
        if self.pending_go_back {
            self.pending_go_back = false;
            // Persist session before leaving the PR
            #[cfg(not(target_arch = "wasm32"))]
            self.save_session();
            self.view = PrReviewView::List;
            self.current_pr = None;
            self.pr_files.clear();
            self.file_diffs.clear();
            self.review_comments.clear();
            self.cached_threads.clear();
            self.issue_comments.clear();
            self.check_runs.clear();
            self.clear_review_state();
            self.collapsed_dirs.clear();
            self.expanded_comment_files.clear();
            self.resolved_thread_lines.clear();
            self.thread_node_ids.clear();
            self.show_only_unresolved = false;
            self.diff_renderer.reset_for_file_change();
            self.diff_renderer.close_search();
        }

        // Auto-fetch PR list on first render if we have a stable token.
        // Wait for token_stable_frames >= 2 so the git credential token has a
        // chance to arrive and replace the OAuth token before we fire the request.
        if self.pull_requests.is_empty()
            && !self.list_loading
            && self.list_error.is_none()
            && self.token.is_some()
            && self.token_stable_frames >= 2
            && self.view == PrReviewView::List
        {
            self.fetch_pr_list();
        }

        match self.view {
            PrReviewView::List => self.show_list_view(ui),
            PrReviewView::Detail => self.show_detail_view(ui),
        }

        // Show file opener popup (rendered on top)
        use crate::components::util::file_opener::{FileOpenerAction, FileOpenerResult};
        let file_opener_result = self.file_opener.show(ui.ctx(), self.theme);
        if let FileOpenerResult::Selected(action) = file_opener_result {
            match action {
                FileOpenerAction::OpenIn(app) => {
                    if let Some(path) = self.file_opener.file_path() {
                        if let Err(e) = app.execute(path) {
                            log::warn!("Failed to open in {}: {e}", app.name());
                        }
                    }
                }
                FileOpenerAction::CopyPath => {
                    if let Some(path) = self.file_opener.file_path() {
                        ui.ctx().copy_text(path.display().to_string());
                    }
                }
                FileOpenerAction::CopyRelativePath => {
                    if let Some(rel) = self.file_opener.relative_path() {
                        ui.ctx().copy_text(rel.display().to_string());
                    }
                }
            }
        }

        // Persist review session every frame while in detail view.
        // Writes are tiny (<1KB) and the OS page cache absorbs the cost.
        #[cfg(not(target_arch = "wasm32"))]
        if self.view == PrReviewView::Detail {
            self.save_session();
        }
    }

    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        if let Some(pr) = &self.current_pr {
            format!("PR #{}", pr.number)
        } else {
            self.name.clone()
        }
    }

    fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    fn label(&self) -> egui::RichText {
        let icon = egui_nerdfonts::regular::GIT_PULL_REQUEST;
        if let Some(pr) = &self.current_pr {
            egui::RichText::new(format!("{icon} #{} {}", pr.number, pr.title))
        } else {
            egui::RichText::new(format!("{icon} Pull Requests"))
        }
    }

    fn description(&self) -> &str {
        ""
    }

    fn handles_own_navigation(&self) -> bool {
        true
    }

    fn to_pane_config(&self) -> Option<enya_config::PaneConfig> {
        Some(enya_config::PaneConfig {
            query: format!("{}/{}", self.owner, self.repo),
            name: self.name.clone(),
            description: String::new(),
            tag: String::new(),
            unit: String::new(),
            granularity: String::new(),
            visualization: "pr_review".to_string(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pane() -> (tokio::runtime::Runtime, PrReviewPane) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime for test");
        let pane = PrReviewPane::new("owner", "repo", AsyncRuntime::new(runtime.handle().clone()));
        (runtime, pane)
    }

    fn run_with_escape(ctx: &egui::Context, mut f: impl FnMut(&egui::Context)) {
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(raw_input, |ctx| f(ctx));
    }

    #[test]
    fn escape_closes_diff_search_even_when_unfocused() {
        let (_runtime, mut pane) = make_pane();
        pane.view = PrReviewView::Detail;
        pane.focused = false;
        pane.diff_renderer.open_search();
        assert!(pane.diff_renderer.search_active());

        let ctx = egui::Context::default();
        run_with_escape(&ctx, |ctx| pane.handle_keyboard(ctx));

        assert!(!pane.diff_renderer.search_active());
    }

    #[test]
    fn escape_does_not_touch_unfocused_non_search_inputs() {
        let (_runtime, mut pane) = make_pane();
        pane.focused = false;
        pane.filter_active = true;
        pane.filter_query = "abc".to_string();

        let ctx = egui::Context::default();
        run_with_escape(&ctx, |ctx| pane.handle_keyboard(ctx));

        assert!(pane.filter_active);
        assert_eq!(pane.filter_query, "abc");
    }

    #[test]
    fn text_input_active_when_commenting_or_searching() {
        let (_runtime, mut pane) = make_pane();
        assert!(!pane.has_text_input_active());

        pane.commenting_line = Some((0, 0));
        assert!(pane.has_text_input_active());

        pane.commenting_line = None;
        pane.diff_renderer.open_search();
        assert!(pane.has_text_input_active());
    }
}
