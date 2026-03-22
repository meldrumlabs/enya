//! PR Review pane — review GitHub pull requests inside Enya.
//!
//! Provides a full PR review experience: list open PRs, view diffs,
//! add comments, approve/request changes, and integrate with AI agents.

mod detail_view;
mod diff_view;
mod list_view;

use rustc_hash::FxHashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::AsyncRuntime;
use crate::components::util::file_opener::FileOpenerPopup;
use crate::components::util::next_id_usize;
use crate::git::api::{
    self, CheckRun, DraftComment, IssueComment, PrComment, PrFile, PullRequest, ReviewEvent,
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

/// Actions that the workspace needs to handle for the PR review pane.
#[derive(Debug, Clone)]
pub enum PrReviewPaneAction {
    /// No action.
    None,
}

/// Maximum number of PRs to preload after the list is fetched.
const PRELOAD_COUNT: usize = 10;

/// Result types for async operations.
type PrListResult = Result<Vec<PullRequest>, String>;
type PrDetailResult = Result<(PullRequest, Vec<PrFile>, String), String>;
type PrCommentsResult = Result<(Vec<PrComment>, Vec<IssueComment>), String>;
type PrChecksResult = Result<Vec<CheckRun>, String>;
type PrSubmitResult = Result<(), String>;

/// All preloaded data for a single PR.
struct PreloadedPr {
    pr: PullRequest,
    files: Vec<PrFile>,
    file_diffs: Vec<FileDiff>,
    review_comments: Vec<PrComment>,
    issue_comments: Vec<IssueComment>,
    check_runs: Vec<CheckRun>,
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
    issue_comments: Vec<IssueComment>,
    check_runs: Vec<CheckRun>,
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
    submitting_review: bool,
    submit_error: Option<String>,
    submit_success: Option<String>,
    pending_submit: Arc<Mutex<Option<PrSubmitResult>>>,

    // ── File tree ──
    /// Collapsed directory paths in the file tree panel.
    collapsed_dirs: rustc_hash::FxHashSet<String>,

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

    // ── Async infrastructure ──
    http_client: reqwest::Client,
    async_runtime: AsyncRuntime,
    token: Option<String>,
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
            issue_comments: Vec::new(),
            check_runs: Vec::new(),
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
            submitting_review: false,
            submit_error: None,
            submit_success: None,
            pending_submit: Arc::new(Mutex::new(None)),
            collapsed_dirs: rustc_hash::FxHashSet::default(),
            file_opener: FileOpenerPopup::new(),
            repo_root: None,
            pending_open_file_opener: false,
            filter_active: false,
            filter_query: String::new(),
            pending_open_pr: None,
            pending_refresh: false,
            pending_go_back: false,
            focused: false,
            preload_cache: FxHashMap::default(),
            pending_preloads: Vec::new(),
            preload_started: false,
            http_client: reqwest::Client::new(),
            async_runtime,
            token: None,
        }
    }

    /// Set the GitHub access token. Called each frame from workspace.
    pub fn set_token(&mut self, token: Option<String>) {
        self.token = token;
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

    /// Navigate to a specific PR by number. Uses preloaded data if available.
    pub fn open_pr(&mut self, number: u32) {
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
            self.selected_file_index = 0;
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
                Ok((review_comments, issue_comments))
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
                    let file_diffs = crate::git::diff::parse_diff_into_files(&diff);
                    let review_comments =
                        api::get_review_comments(&client, &token, &owner, &repo, number).await?;
                    let issue_comments =
                        api::get_issue_comments(&client, &token, &owner, &repo, number).await?;
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

    /// Poll for async operation results. Called each frame.
    fn poll_results(&mut self) {
        // Poll PR list
        if let Some(result) = self.pending_list.lock().take() {
            self.list_loading = false;
            match result {
                Ok(prs) => {
                    self.pull_requests = prs;
                    self.list_error = None;
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
                    self.file_diffs = crate::git::diff::parse_diff_into_files(&diff);
                    let pr_number = pr.number;
                    let head_sha_empty = pr.head.sha.is_empty();
                    self.current_pr = Some(pr);
                    self.pr_files = files;
                    self.selected_file_index = 0;
                    self.detail_error = None;

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
        if let Some(result) = self.pending_comments.lock().take() {
            match result {
                Ok((review_comments, issue_comments)) => {
                    self.review_comments = review_comments;
                    self.issue_comments = issue_comments;
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
                }
            }
        }
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
                } else if self.commenting_line.is_some() {
                    self.commenting_line = None;
                    self.comment_input.clear();
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
            // Always consume keys that would conflict with workspace navigation
            // when this pane is focused, to prevent accidental pane close/navigate.
            // x (workspace: close pane) — never pass through when focused.
            input.consume_key(egui::Modifiers::NONE, egui::Key::X);
            // u (workspace: undo) — never pass through when focused.
            input.consume_key(egui::Modifiers::NONE, egui::Key::U);

            match self.view {
                PrReviewView::List => {
                    // In list view: j/k navigate, Enter/l open PR.
                    // Escape and h are NOT consumed — they pass through to the
                    // workspace so it can unfocus or navigate to another pane.
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
                }
                PrReviewView::Detail => {
                    // In detail view: Escape/h/Backspace go back to list (consumed).
                    // Once in list, next Escape passes through to workspace.
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::H)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
                    {
                        self.pending_go_back = true;
                    }

                    // Delegate standard diff keys to renderer
                    let action = self.diff_renderer.handle_keyboard(input);
                    match action {
                        DiffKeyAction::NextFile => {
                            let max = self.pr_files.len().saturating_sub(1);
                            self.selected_file_index = (self.selected_file_index + 1).min(max);
                            self.diff_renderer.reset_for_file_change();
                        }
                        DiffKeyAction::PrevFile => {
                            self.selected_file_index = self.selected_file_index.saturating_sub(1);
                            self.diff_renderer.reset_for_file_change();
                        }
                        DiffKeyAction::OpenFile => {
                            self.pending_open_file_opener = true;
                        }
                        DiffKeyAction::CopySelected => {
                            // Copy selected text if any
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
        self.poll_results();

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
            self.issue_comments.clear();
            self.check_runs.clear();
            self.collapsed_threads.clear();
            self.collapsed_dirs.clear();
            self.diff_renderer.reset_for_file_change();
        }

        // Auto-fetch PR list on first render if we have a token
        if self.pull_requests.is_empty()
            && !self.list_loading
            && self.list_error.is_none()
            && self.token.is_some()
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
