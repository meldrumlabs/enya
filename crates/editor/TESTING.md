# Testing Guide for Enya Editor

This document describes the testing architecture and practices for the Enya editor crate.

## Quick Start

```bash
# Run all tests WITHOUT the terminal feature (no zig required!)
cargo nextest run -p enya-editor --no-default-features --features all-languages

# Run all tests WITH terminal feature (requires zig toolchain)
cargo nextest run -p enya-editor --all-features

# Run a specific test module
cargo nextest run -p enya-editor --no-default-features --features all-languages -- keyboard

# Check WASM compatibility (no zig required)
cargo check -p enya-editor --target wasm32-unknown-unknown
```

**Note**: The `terminal` feature requires the zig toolchain to build the ghostty terminal
emulator. If you don't need terminal support, use `--no-default-features --features all-languages`
to skip this dependency.

## Test Architecture

The editor has three layers of testing:

### 1. Unit Tests (Pure Logic)

Located in: `src/workspace/keyboard_logic.rs`, `src/workspace/input.rs`, and other modules.

These tests verify pure functions and data structures without egui dependencies:

- **LeaderKeyState tests**: Timeout behavior, key sequence independence
- **VisualMultiState tests**: Selection management, validation
- **KeyboardDecision tests**: All leader key sequences (Space+*, t*, g*, a*, Ctrl+W*)
- **Navigation blocking**: Modal overlay detection

Example:
```rust
#[test]
fn test_space_f_opens_finder() {
    let decision = determine_space_action(egui::Key::F, true);
    assert_eq!(decision, Some(KeyboardDecision::OpenUnifiedFinder));
}
```

### 2. Integration Tests (egui_kittest)

Located in: `tests/ui_integration.rs`

These tests use [egui_kittest](https://docs.rs/egui_kittest) to verify UI components
render correctly and respond to user input. They create a testing harness that can:

- Simulate keyboard input (key presses, modifiers)
- Simulate mouse interaction (clicks, drags)
- Query UI state via AccessKit
- Capture and compare snapshots

### 3. WASM Compatibility Checks

Run: `cargo check -p enya-editor --target wasm32-unknown-unknown`

Verifies the editor compiles for web browsers. See CLAUDE.md for time handling requirements.

## egui_kittest Overview

[egui_kittest](https://docs.rs/egui_kittest) is a testing framework for egui applications
that enables automated UI testing without a window system.

### Core Concepts

**Harness**: The test harness wraps your UI and manages the event loop:

```rust
use egui_kittest::Harness;

// Create a harness with stateful component
let mut harness = Harness::new_state(
    MyComponent::new(),
    |ctx, state| {
        state.show(ctx);
    },
);
```

**Running Frames**: Process events and render the UI:

```rust
harness.run();  // Run until stable (no more repaints requested)
harness.step(); // Run exactly one frame
```

**Simulating Input**: Keyboard and mouse events:

```rust
// Single key press
harness.press_key(egui::Key::Escape);

// Key with modifiers (e.g., Shift+/)
harness.press_key_modifiers(egui::Key::Slash, egui::Modifiers::SHIFT);

// Key combinations
harness.key_combination(&[egui::Key::Control, egui::Key::W]);
```

**Accessing State**: Read component state after interactions:

```rust
assert!(harness.state().is_open());
```

**Snapshot Testing**: Capture and compare rendered images:

```rust
// Capture snapshot (requires "snapshot" and "wgpu" features)
harness.wgpu_snapshot("test_name");

// Update baselines: UPDATE_SNAPSHOTS=true cargo test
```

### Example Test

```rust
#[test]
fn test_which_key_closes_on_escape() {
    // Setup
    let mut which_key = WhichKey::new();
    which_key.open();

    let mut harness = Harness::new_state(
        which_key,
        |ctx, which_key| { which_key.show(ctx); },
    );

    // First run clears "just_opened" flag
    harness.run();
    assert!(harness.state().is_open());

    // Press Escape
    harness.press_key(egui::Key::Escape);
    harness.run();

    // Verify closed
    assert!(!harness.state().is_open());
}
```

## Requirements

### Terminal Feature (Optional)

The terminal feature (`terminal`) is **enabled by default** but can be disabled:

```toml
# Cargo.toml default features
default = ["all-languages", "terminal"]
```

The terminal emulator (`egui_ghostty`) requires the Zig toolchain to build.
If zig is not available and you try to build with the terminal feature, you'll see:

```
error: failed to run custom build command for `ghostty_vt_sys`
```

### Running Without Zig

Since the terminal feature is optional, you can build and test without zig:

```bash
# Build without terminal (no zig required)
cargo build -p enya-editor --no-default-features --features all-languages

# Run ALL tests without terminal (recommended for CI without zig)
cargo nextest run -p enya-editor --no-default-features --features all-languages

# Check WASM target (no zig needed)
cargo check -p enya-editor --target wasm32-unknown-unknown
```

### Installing Zig (for Terminal Feature)

If you want the terminal feature:

- macOS: `brew install zig`
- Linux: `apt install zig` or download from https://ziglang.org/download/
- Windows: Download from https://ziglang.org/download/

**Note**: The zig build requires the ghostty source files.
Ensure the `vendor/ghostty` submodule is properly initialized:
```bash
git submodule update --init vendor/ghostty
```

### Legacy Instructions

<details>
<summary>Old workarounds (no longer needed with optional terminal feature)</summary>

Previously, to run without zig you had to:

1. **Check WASM target** (no zig needed):
   ```bash
   cargo check -p enya-editor --target wasm32-unknown-unknown
   ```

2. **Run specific tests** that don't trigger ghostty:
   ```bash
   cargo nextest run -p enya-editor -- keyboard_logic
   cargo nextest run -p enya-editor -- input::tests
   ```
</details>

## Test Categories

### Keyboard Navigation

Tests for vim-style keybindings:

| Sequence | Action | Test Location |
|----------|--------|---------------|
| `h/j/k/l` | Navigate focus | `keyboard_logic.rs` |
| `Space+f` | Open unified finder | `keyboard_logic.rs`, `ui_integration.rs` |
| `Space+w` | Open workspace finder | `keyboard_logic.rs` |
| `t5/t1/t3/th/td` | Time range shortcuts | `keyboard_logic.rs` |
| `gd/ga/gf` | Go-to shortcuts | `keyboard_logic.rs` |
| `aw/ae/ay` | Agent operators | `keyboard_logic.rs` |
| `Ctrl+W h/j/k/l` | Move pane | `keyboard_logic.rs` |
| `Ctrl+W v/s` | Split pane | `keyboard_logic.rs` |
| `?` | Which-key overlay | `ui_integration.rs` |

### Leader Key Timeouts

Tests verify the 500ms timeout behavior:

- Key sequences expire after timeout
- Multiple leader keys are independent
- Consuming a key clears its state

### Modal Blocking

Tests verify navigation is blocked when overlays are open:

- Unified finder
- Workspace finder
- Command palette
- Buffer editor
- etc. (11 overlay types total)

## Snapshot Testing

egui_kittest supports visual regression testing via snapshots.

**Setup**:
```toml
# Cargo.toml
[dev-dependencies]
egui_kittest = { version = "0.33.3", features = ["snapshot", "wgpu"] }
```

**Usage**:
```rust
#[test]
fn test_component_renders_correctly() {
    let mut harness = Harness::new_ui(|ui| {
        ui.label("Hello, World!");
    });
    harness.run();
    harness.wgpu_snapshot("hello_world");
}
```

**Updating Snapshots**:
```bash
UPDATE_SNAPSHOTS=true cargo test
# or force update all:
UPDATE_SNAPSHOTS=force cargo test
```

Snapshots are stored in `tests/snapshots/` and should be committed to git.
Add `*.diff.png` to `.gitignore` for diff images.

## Writing New Tests

### Guidelines

1. **Prefer pure logic tests** when possible - they're faster and more reliable
2. **Extract testable logic** from egui-coupled code into separate functions
3. **Use `KeyboardContext`** for keyboard decision tests (no egui::Context needed)
4. **Test edge cases**: timeouts, boundary conditions, invalid input

### Adding a New Keyboard Shortcut Test

1. Add the pure logic test in `keyboard_logic.rs`:
   ```rust
   #[test]
   fn test_new_shortcut() {
       let decision = determine_new_action(egui::Key::X);
       assert_eq!(decision, Some(KeyboardDecision::NewAction));
   }
   ```

2. Optionally add a kittest integration test in `ui_integration.rs`:
   ```rust
   #[test]
   fn test_new_shortcut_in_harness() {
       // ... setup harness with relevant component
       harness.press_key(egui::Key::X);
       harness.run();
       // ... verify state changed
   }
   ```

### Testing Components

For new overlay/widget components:

1. Ensure the component has public `open()`, `close()`, `is_open()` methods
2. Create a harness with `Harness::new_state(component, |ctx, state| { state.show(ctx); })`
3. Test open/close behavior, keyboard shortcuts, and expected state changes

## CI Integration

The test suite runs in CI via:

```bash
just ci
```

This includes:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo machete` (unused dependencies)
- `cargo nextest run --all-features`
- `cargo check -p enya-editor --target wasm32-unknown-unknown`

## Troubleshooting

### "ghostty_vt_sys build failed"

The terminal feature requires zig. Either:
- Install zig: `brew install zig`
- Initialize ghostty submodule: `git submodule update --init vendor/ghostty`

### "egui_kittest not found"

Ensure the dev-dependency is enabled:
```toml
[dev-dependencies]
egui_kittest = { version = "0.33.3", features = ["snapshot", "wgpu"] }
```

### Snapshot test failures

If snapshots changed intentionally:
```bash
UPDATE_SNAPSHOTS=true cargo test
```

Review changes carefully before committing updated snapshots.
