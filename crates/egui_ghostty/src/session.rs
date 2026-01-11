//! Terminal session management with PTY integration.

use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use ghostty_vt::{Rgb, Terminal};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::config::TerminalConfig;

/// A terminal session that manages PTY communication.
pub struct TerminalSession {
    /// The ghostty terminal emulator.
    terminal: Terminal,
    /// The PTY master.
    pty_master: Box<dyn MasterPty + Send>,
    /// Writer to the PTY.
    pty_writer: Box<dyn Write + Send>,
    /// The child process.
    #[allow(dead_code)]
    child: Box<dyn Child + Send + Sync>,
    /// Receiver for PTY output.
    output_rx: Receiver<Vec<u8>>,
    /// Handle to the reader thread.
    #[allow(dead_code)]
    reader_thread: JoinHandle<()>,
    /// Terminal title from OSC sequences.
    title: String,
    /// Current dimensions.
    cols: u16,
    rows: u16,
}

impl TerminalSession {
    /// Create a new terminal session with the specified shell.
    pub fn new(config: &TerminalConfig, shell: &str) -> Result<Self, SessionError> {
        // Create the PTY
        let pty_system = native_pty_system();
        let pty_pair = pty_system
            .openpty(PtySize {
                rows: config.rows,
                cols: config.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| SessionError::PtyCreation(e.to_string()))?;

        // Build the shell command as a login shell
        let mut cmd = CommandBuilder::new(shell);
        // Pass -l for login shell to load .zshrc/.bashrc/etc. for proper prompt
        cmd.arg("-l");
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        // Pass through important environment variables
        if let Ok(home) = std::env::var("HOME") {
            cmd.env("HOME", home);
        }
        if let Ok(user) = std::env::var("USER") {
            cmd.env("USER", user);
        }
        if let Ok(shell_env) = std::env::var("SHELL") {
            cmd.env("SHELL", shell_env);
        }
        if let Ok(path) = std::env::var("PATH") {
            cmd.env("PATH", path);
        }
        if let Ok(lang) = std::env::var("LANG") {
            cmd.env("LANG", lang);
        }
        // Set the working directory
        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }

        // Spawn the shell
        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| SessionError::SpawnFailed(e.to_string()))?;

        // Get the PTY master for reading/writing
        let pty_master = pty_pair.master;
        let pty_writer = pty_master
            .take_writer()
            .map_err(|e| SessionError::PtyCreation(e.to_string()))?;

        // Create the terminal emulator
        let mut terminal = Terminal::new(config.cols, config.rows)
            .map_err(|e| SessionError::TerminalCreation(e.to_string()))?;
        terminal.set_default_colors(config.default_fg, config.default_bg);

        // Set up async PTY reading
        let (output_tx, output_rx) = mpsc::channel();
        let mut pty_reader = pty_master
            .try_clone_reader()
            .map_err(|e| SessionError::PtyCreation(e.to_string()))?;

        let reader_thread = thread::spawn(move || {
            Self::read_pty_loop(&mut pty_reader, output_tx);
        });

        Ok(Self {
            terminal,
            pty_master,
            pty_writer,
            child,
            output_rx,
            reader_thread,
            title: String::from("Terminal"),
            cols: config.cols,
            rows: config.rows,
        })
    }

    /// Background thread that reads from PTY and sends to channel.
    fn read_pty_loop(reader: &mut Box<dyn Read + Send>, tx: Sender<Vec<u8>>) {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break; // Channel closed
                    }
                }
                Err(_) => break,
            }
        }
    }

    /// Process any pending output from the PTY.
    ///
    /// Returns true if any output was processed.
    pub fn process_output(&mut self) -> bool {
        let mut had_output = false;

        // Drain all pending output
        while let Ok(data) = self.output_rx.try_recv() {
            if let Err(e) = self.terminal.feed(&data) {
                log::warn!("Failed to feed terminal: {e}");
            }
            had_output = true;
        }

        had_output
    }

    /// Write data to the PTY (keyboard input, etc.).
    pub fn write(&mut self, data: &[u8]) -> Result<(), SessionError> {
        self.pty_writer
            .write_all(data)
            .map_err(|e| SessionError::WriteFailed(e.to_string()))?;
        self.pty_writer
            .flush()
            .map_err(|e| SessionError::WriteFailed(e.to_string()))?;
        Ok(())
    }

    /// Resize the terminal.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), SessionError> {
        if cols == self.cols && rows == self.rows {
            return Ok(());
        }

        // Resize the PTY
        self.pty_master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| SessionError::ResizeFailed(e.to_string()))?;

        // Resize the terminal emulator
        self.terminal
            .resize(cols, rows)
            .map_err(|e| SessionError::ResizeFailed(e.to_string()))?;

        self.cols = cols;
        self.rows = rows;
        Ok(())
    }

    /// Get the terminal emulator for rendering.
    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    /// Get a mutable reference to the terminal emulator.
    pub fn terminal_mut(&mut self) -> &mut Terminal {
        &mut self.terminal
    }

    /// Get the current terminal title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Set the terminal title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    /// Get current dimensions.
    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    /// Set the default colors.
    pub fn set_default_colors(&mut self, fg: Rgb, bg: Rgb) {
        self.terminal.set_default_colors(fg, bg);
    }

    /// Set an ANSI palette color (0-255).
    ///
    /// This updates the terminal's internal palette and triggers a redraw,
    /// causing all cells using this color index to display the new color.
    pub fn set_ansi_color(&mut self, index: u8, color: Rgb) {
        self.terminal.set_ansi_color(index, color);
    }

    /// Scroll the viewport by the specified number of lines.
    ///
    /// Positive values scroll up (show older content in scrollback),
    /// negative values scroll down (show newer content).
    pub fn scroll(&mut self, lines: i32) -> Result<(), SessionError> {
        // ghostty: positive = scroll down = show older content
        // Our API: positive = scroll up = show older content
        // Both want positive for "older", so pass through directly
        self.terminal
            .scroll_viewport(lines)
            .map_err(|e| SessionError::ResizeFailed(e.to_string()))
    }

    /// Scroll to the top of the scrollback buffer.
    pub fn scroll_to_top(&mut self) -> Result<(), SessionError> {
        self.terminal
            .scroll_viewport_top()
            .map_err(|e| SessionError::ResizeFailed(e.to_string()))
    }

    /// Scroll to the bottom (most recent output).
    pub fn scroll_to_bottom(&mut self) -> Result<(), SessionError> {
        self.terminal
            .scroll_viewport_bottom()
            .map_err(|e| SessionError::ResizeFailed(e.to_string()))
    }
}

/// Errors that can occur in terminal sessions.
#[derive(Debug)]
pub enum SessionError {
    /// Failed to create the PTY.
    PtyCreation(String),
    /// Failed to spawn the shell process.
    SpawnFailed(String),
    /// Failed to create the terminal emulator.
    TerminalCreation(String),
    /// Failed to write to the PTY.
    WriteFailed(String),
    /// Failed to resize the terminal.
    ResizeFailed(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::PtyCreation(e) => write!(f, "PTY creation failed: {e}"),
            SessionError::SpawnFailed(e) => write!(f, "Shell spawn failed: {e}"),
            SessionError::TerminalCreation(e) => write!(f, "Terminal creation failed: {e}"),
            SessionError::WriteFailed(e) => write!(f, "PTY write failed: {e}"),
            SessionError::ResizeFailed(e) => write!(f, "Terminal resize failed: {e}"),
        }
    }
}

impl std::error::Error for SessionError {}
