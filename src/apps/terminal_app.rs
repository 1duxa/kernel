//! # Terminal Application
//!
//! Interactive terminal/shell application providing a command-line
//! interface for the kernel.
//!
//! ## Features
//!
//! - Command input and execution
//! - Multi-line input support (Enter adds line, Shift+Enter executes)
//! - Backspace handling with boundary protection
//! - Clear screen (Ctrl+L)
//! - Mouse event display
//!
//! ## Commands
//!
//! Commands are executed via `CommandExecutor`. See `cmd_executor`
//! module for available commands.
//!
//! ## Keyboard Shortcuts
//!
//! - `Enter`: New line
//! - `Shift+Enter`: Execute command
//! - `Ctrl+L`: Clear screen
//! - `Backspace`: Delete character (respects prompt boundary)
//!
//! ## Integration
//!
//! The terminal app wraps a `Terminal` widget and manages:
//! - Input buffering in `current_line`
//! - Prompt display and tracking
//! - Command execution results

use crate::app::{App, AppEvent, FocusBlock};
use crate::cmd_executor::CommandExecutor;
use crate::data_structures::vec::String;
use crate::devices::framebuffer::framebuffer::FramebufferWriter;
use crate::terminal_v2::Terminal;
use crate::ui::theme::Theme;
use crate::ui::widgets::Rect;

pub struct TerminalApp {
    pub term: Terminal,
    block: FocusBlock,
    bounds: Rect,
    current_line: String,
}

impl TerminalApp {
    pub fn new(term: Terminal) -> Self {
        Self {
            term,
            block: FocusBlock {
                id: 1,
                rect: Rect::new(0, 0, 0, 0),
            },
            bounds: Rect::new(0, 0, 0, 0),
            current_line: String::new(),
        }
    }
}

impl TerminalApp {
    fn execute_command(&mut self) {
        let input = self.current_line.clone();
        self.current_line.clear();
        
        self.term.write("\n");
        
        use crate::cmd_executor::CommandResult;
        match CommandExecutor::execute(&input) {
            CommandResult::Output(output) => {
                self.term.write(&output);
                self.term.write("\n");
            }
            CommandResult::Error(error) => {
                let mut err_display = String::from("Error: ");
                err_display.push_str(&error);
                self.term.write(&err_display);
                self.term.write("\n");
            }
            CommandResult::Exit => {
                self.term.write("Goodbye!\n");
            }
        }
        
        self.term.write("> ");
        self.term.set_prompt_start();
    }
}

impl App for TerminalApp {
    fn init(&mut self) {
        self.term.write("> ");
        self.term.set_prompt_start();
    }
    fn on_event(&mut self, event: AppEvent) {
        if let AppEvent::Mouse(_me) = event {
            // For now, just indicate a mouse event was received
            self.term.write("[mouse]");
            return;
        }
        if let AppEvent::KeyPress {
            ch,
            ctrl,
            alt: _,
            shift,
            arrow,
        } = event
        {
            if arrow.is_some() {
                return;
            }
            if ctrl && ch == 'l' {
                self.term.clear();
                self.term.write("> ");
                self.term.set_prompt_start();
                self.current_line.clear();
                return;
            }
            if ch == '\n' {
                if shift {
                    self.execute_command();
                    self.term.set_prompt_start();
                } else {
                    self.term.write("\n");
                    self.current_line.push('\n');
                }
                return;
            }
            if ch == '\x08' {
                self.term.write("\x08");
                if !self.current_line.is_empty() {
                    self.current_line.pop();
                }
                return;
            }
            if !ch.is_control() {
                let mut buf = [0u8; 4];
                self.term.write(ch.encode_utf8(&mut buf));
                self.current_line.push(ch);
            }
        }
    }
    fn layout(&mut self, bounds: Rect) {
        self.bounds = bounds;
        self.block.rect = bounds;
    }
    fn render(&mut self, fb: &mut FramebufferWriter, _theme: &Theme) {
        self.term.render_into_rect(
            fb,
            self.bounds.x,
            self.bounds.y,
            self.bounds.w,
            self.bounds.h,
        );
    }
    fn overlay(&mut self, fb: &mut FramebufferWriter, _theme: &Theme) {
        self.term.draw_cursor(fb, self.bounds.x, self.bounds.y);
    }
    fn focus_blocks(&mut self) -> &mut [FocusBlock] {
        core::slice::from_mut(&mut self.block)
    }
    fn bounds(&self) -> Rect {
        self.bounds
    }
}
