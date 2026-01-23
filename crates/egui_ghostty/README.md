# egui_ghostty

An egui terminal widget backed by [libghostty](https://github.com/ghostty-org/ghostty)'s virtual terminal emulator.

## Features

- Full terminal emulation via ghostty-vt
- PTY integration with portable-pty
- Keyboard and mouse input handling
- Scrollback buffer support
- Dynamic theme updates (ANSI palette colors update in real-time)
- Selection and copy support

## Usage

```rust
use egui_ghostty::{TerminalWidget, ColorScheme};

// Create a terminal with default shell
let mut terminal = TerminalWidget::new("/bin/zsh").unwrap();

// In your egui update loop
terminal.show(ui);

// Update theme colors (affects running TUI apps immediately)
terminal.set_color_scheme(ColorScheme::nord());
```

## Architecture

```
egui_ghostty/
├── lib.rs       # Public API
├── widget.rs    # TerminalWidget - egui rendering
├── session.rs   # TerminalSession - PTY management
├── input.rs     # Keyboard/mouse encoding
├── colors.rs    # ANSI color schemes
└── config.rs    # Configuration options
```

## Acknowledgements

This crate is heavily inspired by [gpui-ghostty](https://github.com/aspect-build/aspect/tree/main/aspect-app/crates/gpui-ghostty) by [Xuanwo](https://github.com/Xuanwo). The ghostty_vt and ghostty_vt_sys crates were adapted from that project.

## License

MIT
