use crate::app::{App, AppEvent, FocusBlock};
use crate::framebuffer::framebuffer::FramebufferWriter;
use crate::terminal::Terminal;
use crate::ui::theme::Theme;
use crate::ui::widgets::Rect;

pub struct TerminalApp {
    pub term: Terminal,
    block: FocusBlock,
    bounds: Rect,
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
        }
    }
}

impl App for TerminalApp {
    fn init(&mut self) {
        self.term.write("> ");
        self.term.set_prompt_start();
    }
    fn on_event(&mut self, event: AppEvent) {
        if let AppEvent::KeyPress {
            ch,
            ctrl,
            alt: _,
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
                return;
            }
            if ch == '\n' {
                self.term.write("\n> ");
                self.term.set_prompt_start();
                return;
            }
            if ch == '\x08' {
                self.term.write("\x08");
                return;
            }
            if !ch.is_control() {
                let mut buf = [0u8; 4];
                self.term.write(ch.encode_utf8(&mut buf));
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
