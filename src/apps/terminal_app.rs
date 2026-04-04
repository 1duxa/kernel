use crate::app::{App, AppEvent, FocusBlock};
use crate::cmd_executor::CommandExecutor;

use crate::terminal_v2::Terminal;
use crate::ui_provider::{render::RenderList, shape::Rect, theme::Theme};
use alloc::string::String;

pub struct TerminalApp {
    terminal: Terminal,
    block: FocusBlock,
    bounds: Rect,
    current_line: String,
    full_redraw: bool,
}

impl TerminalApp {
    pub fn new(width: usize, height: usize) -> Self {
        let cols = (width / 10).max(1);
        let rows = (height / 20).max(1);
        let theme = Theme::dark_modern();

        Self {
            terminal: Terminal::new(cols, rows, &theme),
            block: FocusBlock {
                id: 1,
                rect: Rect::new(0, 0, 0, 0),
            },
            bounds: Rect::new(0, 0, 0, 0),
            current_line: String::new(),
            full_redraw: true,
        }
    }

    fn write_prompt(&mut self) {
        self.terminal.write("> ");
        self.terminal.set_prompt_start();
    }

    fn execute_command(&mut self) {
        let input = self.current_line.clone();
        self.current_line.clear();

        self.terminal.write("\n");

        use crate::cmd_executor::CommandResult;
        match CommandExecutor::execute(&input) {
            CommandResult::Output(output) => {
                self.terminal.write(&output);
                self.terminal.write("\n");
            }
            CommandResult::Error(error) => {
                let mut err_display = String::from("Error: ");
                err_display.push_str(&error);
                self.terminal.write(&err_display);
                self.terminal.write("\n");
            }
            CommandResult::Exit => {
                self.terminal.write("Goodbye!\n");
            }
        }

        self.write_prompt();
    }

    fn clear_screen(&mut self) {
        self.terminal.clear();
        self.current_line.clear();
        self.write_prompt();
        self.full_redraw = true;
    }

    fn resize_terminal(&mut self, theme: &Theme) {
        let cols = (self.bounds.w / 10).max(1);
        let rows = (self.bounds.h / 20).max(1);

        let mut new_terminal = Terminal::new(cols, rows, theme);
        new_terminal.write("Terminal\n");
        new_terminal.write("Type 'help' for available commands\n");
        new_terminal.write("Shortcuts: Alt+Tab to switch apps\n\n");
        self.write_prompt_into(&mut new_terminal);

        self.terminal = new_terminal;
        self.current_line.clear();
        self.full_redraw = true;
    }

    fn write_prompt_into(&self, terminal: &mut Terminal) {
        terminal.write("> ");
        terminal.set_prompt_start();
    }
}

impl App for TerminalApp {
    fn init(&mut self) {
        self.terminal.write("Terminal\n");
        self.terminal.write("Type 'help' for available commands\n");
        self.terminal.write("Shortcuts: Alt+Tab to switch apps\n\n");
        self.write_prompt();
        self.full_redraw = true;
    }

    fn on_event(&mut self, event: AppEvent) -> bool {
        match event {
            AppEvent::Mouse(me) => {
                if me.buttons != 0 {
                    let (mx, my) = crate::devices::mouse_cursor::get_position();
                    self.terminal.write("[click@");
                    self.terminal.write(&format_num(mx));
                    self.terminal.write(",");
                    self.terminal.write(&format_num(my));
                    self.terminal.write("]");
                    true
                } else {
                    false
                }
            }
            AppEvent::KeyPress {
                ch,
                ctrl,
                alt: _,
                shift,
                arrow,
            } => {
                if arrow.is_some() {
                    return false;
                }

                if ctrl && ch == 'l' {
                    self.clear_screen();
                    return true;
                }

                if ch == '\n' {
                    if shift {
                        self.execute_command();
                    } else {
                        self.terminal.write("\n");
                        self.current_line.push('\n');
                    }
                    return true;
                }

                if ch == '\x08' {
                    if !self.current_line.is_empty() {
                        self.terminal.write("\x08");
                        self.current_line.pop();
                        return true;
                    }
                    return false;
                }

                if !ctrl && !ch.is_control() {
                    let mut buf = [0u8; 4];
                    self.terminal.write(ch.encode_utf8(&mut buf));
                    self.current_line.push(ch);
                    return true;
                }

                false
            }
            AppEvent::Tick => false,
        }
    }

    fn layout(&mut self, bounds: Rect) {
        let changed = self.bounds.x != bounds.x
            || self.bounds.y != bounds.y
            || self.bounds.w != bounds.w
            || self.bounds.h != bounds.h;

        self.bounds = bounds;
        self.block.rect = bounds;

        if changed {
            let theme = Theme::dark_modern();
            self.resize_terminal(&theme);
        }
    }

    fn collect_render(
        &mut self,
        theme: &Theme,
        out: &mut RenderList,
    ) {
        if self.full_redraw {
            out.fill_rect(self.bounds, theme.surface);
            self.terminal.collect_render_full(out, self.bounds.x, self.bounds.y);
        } else {
            self.terminal.collect_render(
                out,
                self.bounds.x,
                self.bounds.y,
                self.bounds.w,
                self.bounds.h,
            );
        }
    }

    fn focus_blocks(&mut self) -> &mut [FocusBlock] {
        core::slice::from_mut(&mut self.block)
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// Simple integer-to-string for mouse coordinates (no alloc formatting).
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
