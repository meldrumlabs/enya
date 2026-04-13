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
type PrCommentsResult = Result<(Vec<PrComment>, Vec<IssueComment>, Vec<PrReview>), String>;
type PrChecksResult = Result<Vec<CheckRun>, String>;
type PrSubmitResult = Result<(), String>;
type PrMergeResult = Result<(), String>;
type SingleCommentResult = Result<PrComment, String>;
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

    // ── Conversation tab ──
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

    // ── Per-file reviewed status ──
    /// File paths the user has marked as "reviewed".
    reviewed_files: rustc_hash::FxHashSet<String>,

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
            selected_file_index: 0,
            active_tab: DetailTab::Files,
            detail_loading: false,
            detail_error: None,
            pending_detail: Arc::new(Mutex::new(None)),
            pending_comments: Arc::new(Mutex::new(None)),
            pending_checks: Arc::new(Mutex::new(None)),
            diff_renderer: DiffRenderer::new("pr_diff", typography::SM),
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
            conv_description_collapsed: false,
            collapsed_dirs: rustc_hash::FxHashSet::default(),
            file_tree_scroll_to_selected: false,
            file_panel_collapsed: false,
            seen_comment_ids: rustc_hash::FxHashSet::default(),
            reviewed_files: rustc_hash::FxHashSet::default(),
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
        if token != self.token {
            // Token changed — reset the stability counter so we don't fire a
            // request with a token that's about to be replaced (e.g., OAuth
            // arriving first, then git credential replacing it).
            self.token_stable_frames = 0;
            // If we already have an error from a previous token, clear it so
            // the auto-fetch can retry with the new one.
            if token.is_some() && self.list_error.is_some() {
                self.list_error = None;
            }
        } else if self.token_stable_frames < 2 {
            self.token_stable_frames += 1;
        }
        self.token = token;
    }

    /// Set the AI model ID from user settings.
    pub fn set_ai_model(&mut self, model: Option<String>) {
        self.ai_model = model;
    }

    /// Set whether this pane is the focused tile. Called each frame from workspace.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
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

    /// Rebuild the cached comment threads from `review_comments`.
    fn rebuild_thread_cache(&mut self) {
        self.cached_threads = crate::git::api::group_into_threads(&self.review_comments);
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
                    let diff = api::get_pull_diff(&client, &token, &owner, &repo, number).await?;
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
                Ok((review_comments, issue_comments, reviews))
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
                    let diff = api::get_pull_diff(&client, &token, &owner, &repo, number).await?;
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
        let merge_method = self.merge_method;
        let pending = Arc::clone(&self.pending_merge);

        self.async_runtime.spawn(async move {
            let result =
                api::merge_pull(&client, &token, &owner, &repo, number, None, merge_method).await;
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
                Ok((review_comments, issue_comments, reviews)) => {
                    self.review_comments = review_comments;
                    self.rebuild_thread_cache();
                    self.issue_comments = issue_comments;
                    self.reviews = reviews.clone();
                    if let Some(pr) = &self.current_pr {
                        self.preloaded_reviews.insert(pr.number, reviews);
                    }
                    self.fetch_avatars_for_comments();
                }
                Err(e) => {
                    log::warn!("Failed to fetch PR comments: {e}");
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
                Ok(()) => {
                    self.submit_success = Some("Pull request merged".to_string());
                    self.flash_start = Some(crate::util::Instant::now());
                    self.flash_is_success = true;
                    self.merge_popup_open = false;
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
    /// Only consumes keys when the pane is the focused tile. In list view,
    /// Escape is not consumed so the workspace can unfocus the pane. In
    /// detail view, Escape/h go back to list first.
    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        if !self.focused {
            return;
        }

        // Don't consume keys if a text input is focused (e.g. comment input, filter bar)
        if ctx.memory(|m| m.focused().is_some()) {
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
                }
            }
            return;
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

                    // / — activate filter
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::Slash) {
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

                    // Delegate standard diff keys to renderer
                    let action = self.diff_renderer.handle_keyboard(input);
                    match action {
                        DiffKeyAction::NextFile => {
                            let max = self.file_diffs.len().saturating_sub(1);
                            self.selected_file_index = (self.selected_file_index + 1).min(max);
                            self.diff_renderer.reset_for_file_change();
                            self.file_tree_scroll_to_selected = true;
                            self.mark_current_file_comments_seen();
                        }
                        DiffKeyAction::PrevFile => {
                            self.selected_file_index = self.selected_file_index.saturating_sub(1);
                            self.diff_renderer.reset_for_file_change();
                            self.file_tree_scroll_to_selected = true;
                            self.mark_current_file_comments_seen();
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
