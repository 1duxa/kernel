use crate::devices::framebuffer::color::Color;
use crate::devices::framebuffer::framebuffer::FramebufferWriter;
use crate::ui::theme::Theme;
use crate::ui::widgets::Rect;
use alloc::boxed::Box;
use alloc::vec::Vec;

pub mod navigation;

#[derive(Clone, Copy, Debug)]
pub enum Arrow {
    Up,
    Down,
    Left,
    Right,
}

pub enum AppEvent {
    KeyPress {
        ch: char,
        ctrl: bool,
        alt: bool,
        shift: bool,
        arrow: Option<Arrow>,
    },
    Tick,
}

#[derive(Clone, Copy)]
pub struct FocusBlock {
    pub id: u32,
    pub rect: Rect,
}

pub trait App {
    fn init(&mut self) {}
    fn on_event(&mut self, _event: AppEvent) {}
    fn layout(&mut self, _bounds: Rect) {}
    fn render(&mut self, fb: &mut FramebufferWriter, theme: &Theme);
    fn overlay(&mut self, _fb: &mut FramebufferWriter, _theme: &Theme) {}
    fn focus_blocks(&mut self) -> &mut [FocusBlock];
    fn bounds(&self) -> Rect;
}

pub struct AppHost {
    apps: Vec<Box<dyn App>>,
    focus_app: usize,
    focus_block_id: u32,
}

impl AppHost {
    pub fn new() -> Self {
        Self {
            apps: Vec::new(),
            focus_app: 0,
            focus_block_id: 1,
        }
    }
    pub fn register_app(&mut self, app: Box<dyn App>) {
        if self.apps.is_empty() {
            self.focus_block_id = 1;
        }
        self.apps.push(app);
    }
    pub fn app_mut(&mut self, idx: usize) -> &mut dyn App {
        &mut *self.apps[idx]
    }
    pub fn layout_app(&mut self, idx: usize, bounds: Rect) {
        self.apps[idx].layout(bounds);
    }
    pub fn render_app_once(&mut self, idx: usize, fb: &mut FramebufferWriter, theme: &Theme) {
        self.apps[idx].render(fb, theme);
    }
    pub fn dispatch_event(
        &mut self,
        fb: &mut FramebufferWriter,
        theme: &Theme,
        event: AppEvent,
        accent: Color,
    ) {
        match event {
            AppEvent::KeyPress {
                ch: _,
                ctrl,
                alt,
                shift: _,
                arrow: Some(dir),
            } if ctrl || alt => {
                let blocks = self.apps[self.focus_app].focus_blocks().to_vec();
                self.focus_block_id = navigation::move_focus(&blocks, self.focus_block_id, dir);
            }
            _ => {
                self.apps[self.focus_app].on_event(event);
            }
        }
        self.apps[self.focus_app].render(fb, theme);
        fb.render_frame();
        self.apps[self.focus_app].overlay(fb, theme);
        let blocks = self.apps[self.focus_app].focus_blocks().to_vec();
        if let Some(b) = blocks.iter().find(|b| b.id == self.focus_block_id) {
            navigation::draw_focus_ring(fb, b.rect, accent);
        }
    }
}
