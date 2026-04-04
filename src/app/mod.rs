//! # Application Framework
//!
//! GUI-like application model

use crate::devices::drivers::MouseEvent;
use crate::ui_provider::{
    color::Color,
    render::{flush_commands, RenderCommand, RenderList},
    shape::Rect,
    theme::Theme,
};
use alloc::boxed::Box;
use alloc::vec::Vec;

pub mod navigation;

const OFF_SCREEN_PARK_X: usize = 10_000;

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
    Mouse(MouseEvent),
}

#[derive(Clone, Copy)]
pub struct FocusBlock {
    pub id: u32,
    pub rect: Rect,
}

pub trait App {
    fn init(&mut self) {}
    fn on_event(&mut self, _event: AppEvent) -> bool {
        false
    }
    fn layout(&mut self, _bounds: Rect) {}

    fn collect_render(&mut self, _theme: &Theme, _out: &mut RenderList) {}

    fn collect_overlay(&mut self, _theme: &Theme, _out: &mut RenderList) {}

    fn focus_blocks(&mut self) -> &mut [FocusBlock];
    fn bounds(&self) -> Rect;
}

pub struct AppHost {
    apps: Vec<Box<dyn App>>,
    focus_app: usize,
    focus_block_id: u32,
    render_commands: RenderList,
    overlay_commands: RenderList,
    needs_redraw: bool,
}

impl AppHost {
    pub fn new() -> Self {
        Self {
            apps: Vec::new(),
            focus_app: 0,
            focus_block_id: 1,
            render_commands: RenderList::new(),
            overlay_commands: RenderList::new(),
            needs_redraw: true,
        }
    }

    pub fn register_app(&mut self, app: Box<dyn App>) {
        if self.apps.is_empty() {
            self.focus_block_id = 1;
        }
        self.apps.push(app);
        self.request_redraw();
    }

    pub fn app_mut(&mut self, idx: usize) -> &mut dyn App {
        &mut *self.apps[idx]
    }

    pub fn layout_app(&mut self, idx: usize, bounds: Rect) {
        self.apps[idx].layout(bounds);
        self.request_redraw();
    }

    pub fn render_app_once(&mut self, idx: usize, theme: &Theme) {
        self.render_commands.clear();
        self.apps[idx].collect_render(theme, &mut self.render_commands);
    }

    pub fn render_focused_app(&mut self, theme: &Theme) {
        if self.focus_app >= self.apps.len() {
            return;
        }

        self.render_commands.clear();
        self.apps[self.focus_app].collect_render(theme, &mut self.render_commands);

        self.overlay_commands.clear();
        self.apps[self.focus_app].collect_overlay(theme, &mut self.overlay_commands);
        self.draw_focus_ring(Color::from_hex(0xFF6B6B));

        self.needs_redraw = false;
    }

    pub fn render_all_apps(&mut self, theme: &Theme) {
        self.render_commands.clear();

        for i in 0..self.apps.len() {
            self.apps[i].collect_render(theme, &mut self.render_commands);
        }

        if self.focus_app < self.apps.len() {
            self.overlay_commands.clear();
            self.apps[self.focus_app].collect_overlay(theme, &mut self.overlay_commands);
            self.draw_focus_ring(Color::from_hex(0xFF6B6B));
        } else {
            self.overlay_commands.clear();
        }

        self.needs_redraw = false;
    }

    pub fn cycle_focus(&mut self) {
        if self.apps.is_empty() {
            return;
        }
        self.focus_app = (self.focus_app + 1) % self.apps.len();
        let blocks = self.apps[self.focus_app].focus_blocks();
        if !blocks.is_empty() {
            self.focus_block_id = blocks[0].id;
        }
        self.request_redraw();
    }

    pub fn switch_to_app(&mut self, idx: usize) -> bool {
        if idx < self.apps.len() {
            self.focus_app = idx;
            let blocks = self.apps[self.focus_app].focus_blocks();
            if !blocks.is_empty() {
                self.focus_block_id = blocks[0].id;
            }
            self.request_redraw();
            true
        } else {
            false
        }
    }

    pub fn handle_mouse_click(&mut self, x: usize, y: usize) {
        for (idx, app) in self.apps.iter().enumerate() {
            let bounds = app.bounds();
            if x >= bounds.x && x < bounds.x + bounds.w && y >= bounds.y && y < bounds.y + bounds.h
            {
                if idx != self.focus_app {
                    self.focus_app = idx;
                    let blocks = self.apps[self.focus_app].focus_blocks();
                    if !blocks.is_empty() {
                        self.focus_block_id = blocks[0].id;
                    }
                    self.request_redraw();
                }
                break;
            }
        }
    }

    pub fn app_count(&self) -> usize {
        self.apps.len()
    }

    pub fn focused_app_index(&self) -> usize {
        self.focus_app
    }

    pub fn render_commands(&self) -> &[RenderCommand] {
        self.render_commands.as_slice()
    }

    pub fn overlay_commands(&self) -> &[RenderCommand] {
        self.overlay_commands.as_slice()
    }

    pub fn dispatch_event(&mut self, event: AppEvent) {
        if self.apps.is_empty() {
            return;
        }

        let changed = match event {
            AppEvent::KeyPress {
                ch: _,
                ctrl,
                alt,
                shift: _,
                arrow: Some(dir),
            } if ctrl || alt => {
                let blocks = self.apps[self.focus_app].focus_blocks().to_vec();
                let next_focus = navigation::move_focus(&blocks, self.focus_block_id, dir);
                let changed = next_focus != self.focus_block_id;
                self.focus_block_id = next_focus;
                changed
            }
            _ => self.apps[self.focus_app].on_event(event),
        };

        if changed {
            self.request_redraw();
        }
    }

    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
    }

    pub fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    pub fn compose(&mut self, theme: &Theme, accent: Color) {
        self.render_commands.clear();

        for i in 0..self.apps.len() {
            if self.apps[i].bounds().x >= OFF_SCREEN_PARK_X {
                continue;
            }
            self.apps[i].collect_render(theme, &mut self.render_commands);
        }

        self.overlay_commands.clear();
        if self.focus_app < self.apps.len() {
            self.apps[self.focus_app].collect_overlay(theme, &mut self.overlay_commands);
            self.draw_focus_ring(accent);
        }

        self.needs_redraw = false;
    }

    pub fn flush(&self, fb: &mut crate::devices::framebuffer::framebuffer::FramebufferWriter) {
        flush_commands(fb, self.render_commands.as_slice());
        flush_commands(fb, self.overlay_commands.as_slice());
    }

    fn draw_focus_ring(&mut self, accent: Color) {
        let blocks = self.apps[self.focus_app].focus_blocks().to_vec();
        if let Some(b) = blocks.iter().find(|b| b.id == self.focus_block_id) {
            self.overlay_commands
                .push(RenderCommand::stroke_rect(b.rect, accent, 2));
        }
    }
}
