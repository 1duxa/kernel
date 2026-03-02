//! # Terminal Application
//!
//! Interactive shell with command execution and multi-line input.
//!
//! ## Shortcuts
//!
//! | Key | Action |
//! |-----|--------|
//! | Enter | New line |
//! | Shift+Enter | Execute command |
//! | Ctrl+L | Clear screen |
//! | Backspace | Delete character |

use crate::app::{App, AppEvent, FocusBlock};
use crate::cmd_executor::CommandExecutor;
use crate::devices::framebuffer::framebuffer::FramebufferWriter;
use crate::terminal_v2::Terminal;
use crate::ui::theme::Theme;
use crate::ui::widgets::Rect;
use alloc::string::String;

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
        self.term.write("DuxOS Terminal v2\n");
        self.term.write("Type 'help' for available commands\n");
        self.term
            .write("Shortcuts: Alt+Tab to switch apps, Ctrl+Arrows for navigation\n\n");
        self.term.write("> ");
        self.term.set_prompt_start();
    }

    fn on_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Mouse(me) => {
                // Only show mouse info on clicks, not movement
                if me.buttons != 0 {
                    let (mx, my) = crate::devices::mouse_cursor::get_position();
                    self.term.write("[click@");
                    self.term.write(&format_num(mx));
                    self.term.write(",");
                    self.term.write(&format_num(my));
                    self.term.write("]");
                }
            }
            AppEvent::KeyPress {
                ch,
                ctrl,
                alt: _,
                shift,
                arrow,
            } => {
                // Ignore arrow keys (handled by navigation)
                if arrow.is_some() {
                    return;
                }

                // Ctrl+L: clear screen
                if ctrl && ch == 'l' {
                    self.term.clear();
                    self.term.write("> ");
                    self.term.set_prompt_start();
                    self.current_line.clear();
                    return;
                }

                // Enter key
                if ch == '\n' {
                    if shift {
                        // Shift+Enter: execute command
                        self.execute_command();
                    } else {
                        // Enter: new line in multi-line input
                        self.term.write("\n");
                        self.current_line.push('\n');
                    }
                    return;
                }

                // Backspace
                if ch == '\x08' {
                    if !self.current_line.is_empty() {
                        self.term.write("\x08");
                        self.current_line.pop();
                    }
                    return;
                }

                // Regular character
                if !ch.is_control() {
                    let mut buf = [0u8; 4];
                    self.term.write(ch.encode_utf8(&mut buf));
                    self.current_line.push(ch);
                }
            }
            AppEvent::Tick => {}
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

/// Simple number to string formatting (no alloc formatting)
fn format_num(n: i32) -> String {
    if n == 0 {
        return String::from("0");
    }

    let mut s = String::new();
    let mut num = n;
    let negative = num < 0;
    if negative {
        num = -num;
    }

    let mut digits = [0u8; 12];
    let mut i = 0;
    while num > 0 {
        digits[i] = (num % 10) as u8;
        num /= 10;
        i += 1;
    }

    if negative {
        s.push('-');
    }

    while i > 0 {
        i -= 1;
        s.push((b'0' + digits[i]) as char);
    }

    s
}
