//! # Terminal Logger
//!
//! Provides logging output to both serial and screen terminal.
//!
//! This module creates a dual-output logging system that writes
//! to the serial port (for debugging) and to an on-screen terminal
//! (for user visibility).

use crate::devices::framebuffer::framebuffer::{FramebufferWriter, FRAMEBUFFER};
use crate::terminal_v2::Terminal;
use crate::ui::Theme;
use core::fmt::{self, Write};
use spin::Mutex;

/// Global terminal logger instance
static TERMINAL_LOGGER: Mutex<Option<TerminalLogger>> = Mutex::new(None);

/// Terminal logger that outputs to both serial and screen
pub struct TerminalLogger {
    terminal: Terminal,
}

impl TerminalLogger {
    pub fn new(cols: usize, rows: usize, theme: &Theme) -> Self {
        Self {
            terminal: Terminal::new(cols, rows, theme),
        }
    }

    /// Write a string to the terminal
    pub fn write_str(&mut self, s: &str) {
        self.terminal.write(s);
    }

    /// Render the terminal to framebuffer
    pub fn render(&mut self, fb: &mut FramebufferWriter) {
        self.terminal.render(fb);
    }

    /// Clear the terminal
    pub fn clear(&mut self) {
        self.terminal.clear();
    }

    /// Get terminal reference
    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    /// Get mutable terminal reference
    pub fn terminal_mut(&mut self) -> &mut Terminal {
        &mut self.terminal
    }
}

impl Write for TerminalLogger {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.terminal.write(s);
        Ok(())
    }
}

/// Initialize the terminal logger
pub fn init(cols: usize, rows: usize, theme: &Theme) {
    let logger = TerminalLogger::new(cols, rows, theme);
    *TERMINAL_LOGGER.lock() = Some(logger);
}

/// Write to the terminal logger (if initialized)
pub fn write_str(s: &str) {
    if let Some(ref mut logger) = *TERMINAL_LOGGER.lock() {
        logger.write_str(s);
    }
}

/// Write formatted output to terminal logger
pub fn write_fmt(args: fmt::Arguments) {
    if let Some(ref mut logger) = *TERMINAL_LOGGER.lock() {
        let _ = logger.write_fmt(args);
    }
}

/// Render the terminal logger to screen
pub fn render() {
    if let Some(ref mut logger) = *TERMINAL_LOGGER.lock() {
        let mut guard = FRAMEBUFFER.lock();
        if let Some(fb) = guard.as_mut() {
            logger.render(fb);
            fb.render_frame();
        }
    }
}

/// Clear the terminal logger
pub fn clear() {
    if let Some(ref mut logger) = *TERMINAL_LOGGER.lock() {
        logger.clear();
    }
}

/// Macro for printing to both serial and terminal
#[macro_export]
macro_rules! kprintln {
    () => {
        $crate::println!();
        $crate::terminal_logger::write_str("\n");
    };
    ($($arg:tt)*) => {{
        $crate::println!($($arg)*);
        $crate::terminal_logger::write_fmt(format_args!($($arg)*));
        $crate::terminal_logger::write_str("\n");
    }};
}

/// Macro for printing without newline
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        // Serial output
        {
            use core::fmt::Write;
            let mut serial = $crate::SERIAL.lock();
            let _ = write!(serial, $($arg)*);
        }
        // Terminal output
        $crate::terminal_logger::write_fmt(format_args!($($arg)*));
    }};
}
