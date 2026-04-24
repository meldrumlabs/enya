# PR Review Pane — Premium UX Audit & Recommendations

**Date:** 2026-04-22
**Scope:** `crates/editor/src/components/pane/git/` + diff renderer + overlay
**Inspiration:** Graphite.dev (stacked workflows, AI review, PR inbox), Linear.app (keyboard-first navigation, minimal focused UI, agent integration, structural diffs)

---

## Executive Summary

The current PR Review pane is functionally complete and well-engineered: it has a performant custom diff renderer, floating comment gutters, AI walkthroughs, thread resolution, file trees, check runs, and vim-style keybindings. The gap to a *premium* UX is not missing features — it is **information hierarchy, review momentum, and cognitive load reduction**.

Graphite and Linear both optimize for one thing: **getting the user to the next action as fast as possible with zero ambiguity**. The following recommendations are ordered by estimated impact (highest first) and grouped by theme.

---

## 🏆 Tier 1 — Game-Changers (Do These First)

### 1. Review Inbox with Smart Segments
**Impact: Very High | Effort: Medium**

**Problem:** The PR list is a flat chronological stream. A busy reviewer sees 20+ PRs and has to mentally triage which ones need attention.

**Graphite / Linear Inspiration:**
- Graphite's **PR Inbox** segments by "Needs my review", "Approved", "My PRs", "Blocked".
- Linear's **Triage** automatically surfaces what needs attention.

**Recommendation:**
Replace the single flat list with **segmented inbox tabs** above the list:

```
┌─────────────────────────────────────────┐
│  Needs Review  │  My PRs  │  All  │  /  │
│     4 badges   │   2      │  23   │ 🔍  │
├─────────────────────────────────────────┤
│  #142  Add auth middleware              │
│  #141  Fix race in scheduler           │
│  #138  Update dependency lockfiles       │
└─────────────────────────────────────────┘
```

- **Needs my review:** PRs where I am a requested reviewer, or repo default, AND I have not submitted a review, ordered by urgency (age, check status, comment activity).
- **My PRs:** PRs I authored that are open, with sub-badges for "Approved", "Changes requested", "Failing checks".
- **All:** The current flat list.

**Implementation Notes:**
- This is purely client-side filtering using already-fetched data. The `PullRequest` struct has `user.login` for authorship and we already preload reviews.
- Add a new `PrListSegment` enum to `list_view.rs` and filter inside `filtered_pr_indices()`.
- Consider adding a "snooze" or "done" gesture (swipe or key) to remove a PR from "Needs my review" without actually reviewing — useful for draft PRs or WIP.

---

### 2. Persistent Review Session State
**Impact: Very High | Effort: Low–Medium**

**Problem:** When a reviewer switches files, closes Enya, or gets interrupted, they lose:
- Which files were marked reviewed (`reviewed_files` is in-memory only)
- Which comments were "seen" (`seen_comment_ids` is in-memory only)
- Scroll position per file
- Collapsed/expanded directory state
- Active filter query

**Linear Inspiration:** Linear remembers *everything* — where you were, what you expanded, what you dismissed. It feels like a persistent workspace, not a web page.

**Recommendation:**
Serialize review session state to a local JSON file keyed by `(owner, repo, pr_number, user_login)`:

```json
{
  "reviewed_files": ["src/auth.rs", "src/middleware.rs"],
  "seen_comment_ids": [12345, 12346],
  "file_scroll_offsets": {"src/auth.rs": 420.0},
  "collapsed_dirs": ["tests/"],
  "filter_query": "auth",
  "last_opened_at": "2026-04-22T10:00:00Z"
}
```

**Implementation Notes:**
- Store in `~/.cache/enya/pr_sessions/{owner}_{repo}_{pr}_{user}.json` or similar.
- Load on `open_pr()` and merge with fresh data (new comments get marked unseen, newly added files are unreviewed).
- This transforms the PR pane from a *viewer* into a *workspace*.

---

### 3. Command Palette (Cmd+K) for PR Navigation
**Impact: High | Effort: Medium**

**Problem:** Keyboard shortcuts exist (`j/k`, `n/p`, `gg/G`, etc.) but they are discoverable only via the footer hint. There is no way to *jump* to a specific PR, file, or comment thread without repetitive key presses.

**Linear Inspiration:** Linear is unusable without `Cmd+K`. It is the primary navigation surface for issues, projects, commands, and AI actions.

**Recommendation:**
Add a **PR Review Command Palette** triggered by `Cmd+K` (or `/` when focused on the pane):

```
┌─────────────────────────────────────────┐
│ > jump to file...                       │
│   src/auth.rs                    L42    │
│   src/middleware.rs             L105    │
│   tests/auth_tests.rs            L12    │
│ > jump to comment...                    │
│   alice: "Should we cache this?" auth.rs│
│ > mark all files reviewed               │
│ > submit review                         │
│ > open PR in GitHub                     │
└─────────────────────────────────────────┘
```

**Implementation Notes:**
- Reuse the existing `components/overlay/command_palette.rs` infrastructure if possible.
- Register pane-specific commands when `PrReviewPane` is focused.
- "Jump to file" should search across `file_diffs` by path and jump to the file + scroll offset.
- "Jump to comment" should search across `cached_threads` by author + snippet and call `navigate_to_thread()`.

---

### 4. Review Mode Switcher (Quick Scan vs. Deep Dive)
**Impact: High | Effort: Low**

**Problem:** The Files tab always shows all files in alphabetical/diff order. A reviewer doing a quick safety check on a trusted teammate's small PR has to manually skip test files and generated code.

**Graphite Inspiration:** Graphite's review experience is built around "review smaller PRs faster." The UI should adapt to PR size and reviewer intent.

**Recommendation:**
Add a **Review Mode** toggle in the Files tab header:

```
┌─────────────────────────────────────────┐
│ Files  │ Conversation │ Checks     [?] │
│ [Quick Scan ▼]  Organize   Submit Review │
├─────────────────────────────────────────┤
```

Modes:
- **Deep Dive** (default): All files, full diff, all comments.
- **Quick Scan:** Hide files with no comments and no significant changes (e.g., only lockfile bumps, only import changes). Show files with comments or high-impact changes first.
- **First Pass:** Show docs/README changes first, then model/schema, then API, then tests last (use AI walkthrough grouping if available, else heuristic by path).
- **Changes Only:** Hide files already marked reviewed (allow re-show via filter).

**Implementation Notes:**
- This is a display filter; no backend changes.
- Add `ReviewMode` enum to `PrReviewPane` and filter `file_diffs` before rendering the file tree and diff view.
- The "Changes Only" mode is especially powerful combined with persistent session state — it lets a reviewer resume exactly where they left off.

---

## 🥈 Tier 2 — Strong Differentiators

### 5. Inline Suggested Changes (Accept / Reject)
**Impact: Very High | Effort: High**

**Problem:** Comments that suggest code changes require the author to manually edit, then push, then re-request review. This is the #1 source of review round-trips.

**Graphite / GitHub Inspiration:** GitHub's "Suggest a change" is one of its most loved features. Graphite's AI reviews auto-suggest fixes.

**Recommendation:**
Support **suggested change blocks** in comments. When a comment body contains a GitHub-style suggestion fence:

```suggestion
    let x = new_value;
```

Render it in the comment gutter as an **actionable diff snippet** with two buttons:

```
┌─────────────────────────────────────────┐
│ alice · 2h ago                          │
│ Use `new_value` here instead            │
│ ┌─ Suggested change ─────────────────┐  │
│ │ -    let x = old_value;            │  │
│ │ +    let x = new_value;            │  │
│ │ [Accept] [Dismiss] [Modify...]     │  │
│ └──────────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

**Implementation Notes:**
- Parse ` ```suggestion ` blocks from comment bodies (already returned by GitHub API).
- "Accept" triggers a local workspace edit via the `file_opener` / editor integration, or if Enya has a code-editing agent, applies it automatically.
- "Dismiss" simply marks the thread resolved.
- "Modify" opens the suggestion in an inline text editor for tweaking before applying.
- This requires plumbing between the PR pane and the codebase editor — a significant but highly valuable integration.

---

### 6. Stacked PR Visualization
**Impact: High | Effort: Medium–High**

**Problem:** Enya targets "builders." Builders who use Graphite or `git-branchless` work in stacks. The current pane shows one PR in isolation, losing the critical context of "what depends on this."

**Graphite Inspiration:** Graphite's entire UX is built around the stack. Every PR page shows its position in the stack, parent/child relationships, and whether the stack is green.

**Recommendation:**
Add a **stack strip** to the detail view header when the PR is part of a detected stack:

```
┌─────────────────────────────────────────┐
│ ◀  #140  Base refactor  [merged]       │
│ ●  #141  Add auth middleware  [open] ←  │
│ ○  #142  Wire up frontend  [open]       │
│ ○  #143  Update tests    [draft]        │
└─────────────────────────────────────────┘
```

- Detect stacks by analyzing PR descriptions for `Depends on #140` or branch name patterns (`alice/auth-stack-1`, `alice/auth-stack-2`).
- Fetch sibling PRs in the background when opening a PR.
- Clicking a sibling PR swaps the detail view instantly (using preload cache).
- Show aggregate CI status for the whole stack.

**Implementation Notes:**
- Add `stack_siblings: Option<Vec<PullRequest>>` to detail state.
- Heuristic detection: search PR body for `#\d+`, or branch prefixes before the last `-N` suffix.
- This positions Enya as a tool for advanced Git workflows, not just a GitHub viewer.

---

### 7. Conversation Digest / Thread Summary Banner
**Impact: Medium–High | Effort: Medium**

**Problem:** PRs with 20+ comments require the reviewer to scroll through the entire Conversation tab to understand what is still unresolved. The "unresolved filter" in the file tree is helpful but doesn't tell the *story* of the discussion.

**Linear / Graphite Inspiration:** Linear's agent summarizes issue threads. Graphite's AI review gives a high-level summary.

**Recommendation:**
When a PR has ≥3 unresolved comment threads, show a **sticky digest banner** at the top of the Conversation tab:

```
┌─────────────────────────────────────────┐
│ 3 open threads · 2 resolved · 1 draft │
│ ▼ Unresolved:                         │
│   • alice: "Should we cache this?"     │
│     in src/auth.rs L42                  │
│   • bob: "Missing rate limit test"     │
│     in tests/auth.rs L88               │
│   • You: "Nit: rename to `token`"      │
│     in src/middleware.rs L15            │
└─────────────────────────────────────────┘
```

- Clicking an item jumps directly to the thread in the Files tab (via `navigate_to_thread()`).
- If the AI walkthrough is active, add an AI-generated 1-sentence summary of each thread's status: "Thread on auth.rs L42: awaiting author's response."

**Implementation Notes:**
- Purely a new UI component using existing `cached_threads` data.
- Add to `show_conversation_tab()` above the scroll area.
- Consider also showing this digest in a collapsed sidebar in the Files tab so it's visible while reviewing code.

---

### 8. File-Level Risk / Impact Indicators
**Impact: Medium–High | Effort: Medium**

**Problem:** The file tree shows `+/-` counts and comment badges, but these are noisy signals. A 200-line lockfile change and a 20-line core algorithm change look similar.

**Graphite Inspiration:** Graphite's diff view highlights *signal* — what actually matters.

**Recommendation:**
Enhance the file tree with **impact badges**:

```
┌─────────────────────────────────────────┐
│ src/auth.rs              +45 -12  ⚠️    │
│ src/middleware.rs         +8  -3       │
│ Cargo.lock              +200 -180  🧪   │
│ tests/auth_tests.rs      +60  -0       │
└─────────────────────────────────────────┘
```

- **⚠️ High impact:** Files with comments, or files matched by heuristics (contains `unsafe`, modifies core data structures, changes public API signatures).
- **🧪 Test-only:** Files under `tests/`, `*_test.rs`, etc.
- **🔒 Generated:** Lockfiles, generated protobuf/codegen files.
- **📦 Dependency-only:** Changes limited to `Cargo.toml`, `package.json`, etc.

**Implementation Notes:**
- Heuristics are regex/path-based; no AI needed.
- Add `FileImpact` enum and compute it when building `FileTreeRow`.
- Use small icons (already using Nerd Fonts) to avoid clutter.
- This helps reviewers mentally prioritize without reading every file.

---

## 🥉 Tier 3 — Polish & Delight

### 9. Haptic / Visual Feedback for Review Milestones
**Impact: Medium | Effort: Low**

**Problem:** Marking a file as reviewed (`v` key) auto-advances to the next file, but the transition is instant and jarring. Finishing all files shows a small banner that is easy to miss.

**Linear Inspiration:** Linear celebrates state transitions. Moving an issue to "Done" is satisfying.

**Recommendation:**
- Add a **brief flash animation** when a file is marked reviewed: the file row in the tree briefly glows green, then the diff crossfades to the next file.
- When the last file is reviewed, play a **micro-celebration**: the progress bar fills with a smooth animation, the "All files reviewed" banner slides in with a subtle bounce, and the "Submit Review" button pulses gently.
- Use `ui.ctx().request_repaint()` with a small timer — already used for hunk flash and button flashes, extend the pattern.

---

### 10. Smart Default Split View by File Type
**Impact: Low–Medium | Effort: Very Low**

**Problem:** Split view is toggled globally. Some files (wide tables, prose markdown) benefit from unified; others (structured code) benefit from split.

**Recommendation:**
Remember the user's last choice per file extension, or default to split for `.rs`, `.ts`, `.py` and unified for `.md`, `.toml`, `.json`.

---

### 11. Check Failure Context in File Tree
**Impact: Medium | Effort: Medium**

**Problem:** Checks are in a separate tab. A reviewer looking at a file doesn't know if a test failure is related to that file.

**Recommendation:**
If check runs include annotations (file + line from GitHub Checks API), show a **red dot** on the affected file in the tree and in the diff gutter at the annotated line. Hovering shows the failure message.

---

### 12. Review "Confidence" Badge
**Impact: Low–Medium | Effort: Low**

**Problem:** Small, safe PRs (typo fixes, dependency bumps) still require the same UI ceremony as large risky PRs.

**Recommendation:**
Add a **confidence badge** to the PR detail header:

```
┌─────────────────────────────────────────┐
│ #142  Fix typo in auth error message     │
│            Low Risk · 1 file · +1 -1    │
└─────────────────────────────────────────┘
```

Computed from:
- Lines changed (< 50 = low risk)
- Files touched (1 = low risk)
- Test coverage (if checks include coverage data)
- AI walkthrough concern count (0 = safer)
- Author (trusted frequent contributor vs. first-time)

This is informational only — never auto-approve — but it reduces anxiety for trivial reviews.

---

## Implementation Priority Matrix

| # | Feature | Impact | Effort | Priority |
|---|---------|--------|--------|----------|
| 1 | Review Inbox Segments | ⭐⭐⭐⭐⭐ | M | **P0** |
| 2 | Persistent Session State | ⭐⭐⭐⭐⭐ | M | **P0** |
| 3 | Command Palette (Cmd+K) | ⭐⭐⭐⭐⭐ | M | **P0** |
| 4 | Review Mode Switcher | ⭐⭐⭐⭐ | L | **P1** |
| 5 | Inline Suggested Changes | ⭐⭐⭐⭐⭐ | H | **P1** |
| 6 | Stacked PR Visualization | ⭐⭐⭐⭐ | M–H | **P1** |
| 7 | Conversation Digest Banner | ⭐⭐⭐⭐ | M | **P1** |
| 8 | File-Level Risk Indicators | ⭐⭐⭐⭐ | M | **P2** |
| 9 | Review Milestone Feedback | ⭐⭐⭐ | L | **P2** |
| 10 | Smart Split Defaults | ⭐⭐ | L | **P2** |
| 11 | Check Annotations in Tree | ⭐⭐⭐ | M | **P2** |
| 12 | Confidence Badge | ⭐⭐⭐ | L | **P3** |

---

## Code-Ready Notes

### Where to Touch

| Feature | Primary Files | Notes |
|---------|--------------|-------|
| Inbox Segments | `list_view.rs`, `mod.rs` | Add `PrListSegment` enum, filter in `filtered_pr_indices()` |
| Persistent State | `mod.rs` | Add `save_session()` / `load_session()` calls in `open_pr()` and state mutations |
| Command Palette | New: `pr_command_palette.rs` | Reuse `components/overlay/command_palette.rs` patterns, register in workspace key router |
| Review Modes | `detail_view.rs`, `diff_view.rs` | Add `ReviewMode` enum, filter `file_diffs` before tree/diff render |
| Suggested Changes | `diff_view.rs`, `detail_view.rs` | Parse `` ```suggestion `` in `PrComment::body`, add Accept/Dismiss UI in `render_floating_card()` |
| Stack Viz | `mod.rs`, `detail_view.rs` | Add `stack_siblings` fetch in `open_pr()`, render strip in `show_detail_view()` |
| Digest Banner | `detail_view.rs` | Add `show_conversation_digest()` component above scroll area |
| Risk Indicators | `detail_view.rs` | Extend `FileTreeRow::File` with `impact: FileImpact`, render icon in tree |
| Milestone Feedback | `detail_view.rs` | Extend existing `flash_start` / `scroll_anim` patterns |
| Check Annotations | `detail_view.rs`, `diff_view.rs` | Parse GitHub Checks API annotations, map to file paths and line numbers |

### Patterns to Preserve

The current codebase has excellent patterns that should be extended, not replaced:

- **Deferred actions:** All comment/hunk/file actions are deferred out of closures (e.g., `clicked_file: Option<usize>`) to avoid borrow checker issues. Continue this pattern.
- **Raw painter calls:** The file tree, diff lines, and badges are rendered with direct `ui.painter()` calls for performance. New indicators should follow this, not switch to standard egui widgets.
- **Keyboard-first:** Every new feature should have a keyboard shortcut and appear in the footer hint.
- **Theme-aware:** All colors go through `AppTheme` methods (e.g., `theme.diff_added_text()`) — never hardcode.

---

## Closing Thought

The current PR Review pane is a solid 80% solution. The remaining 20% is what transforms it from a "GitHub viewer inside an editor" into a **review workspace** that users prefer over the browser. Graphite and Linear prove that developers will switch tools for a 10x better workflow. These recommendations target the exact friction points that make code review feel like a chore: triage, context loss, repetitive navigation, and unclear next actions.
