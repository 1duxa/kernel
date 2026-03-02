//! # Application Framework
//!
//! GUI-like application model with event handling and focus management.
//!
//! ## Components
//!
//! - `App` trait: Interface for applications
//! - `AppHost`: Manages multiple apps and dispatches events
//! - `AppEvent`: Keyboard, mouse, and tick events
//! - `FocusBlock`: Focusable UI regions for navigation

use crate::devices::framebuffer::color::Color;
use crate::devices::drivers::MouseEvent;
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
    Mouse(MouseEvent),
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

    /// Render the currently focused app
    pub fn render_focused_app(&mut self, fb: &mut FramebufferWriter, theme: &Theme) {
        if self.focus_app < self.apps.len() {
            self.apps[self.focus_app].render(fb, theme);
            self.apps[self.focus_app].overlay(fb, theme);
            
            // Draw focus ring
            let blocks = self.apps[self.focus_app].focus_blocks().to_vec();
            if let Some(b) = blocks.iter().find(|b| b.id == self.focus_block_id) {
                navigation::draw_focus_ring(fb, b.rect, Color::from_hex(0xFF6B6B));
            }
        }
    }
    
    /// Render all apps (for split-screen layout)
    pub fn render_all_apps(&mut self, fb: &mut FramebufferWriter, theme: &Theme) {
        for i in 0..self.apps.len() {
            self.apps[i].render(fb, theme);
        }
        // Draw focus ring on focused app
        if self.focus_app < self.apps.len() {
            self.apps[self.focus_app].overlay(fb, theme);
            let blocks = self.apps[self.focus_app].focus_blocks().to_vec();
            if let Some(b) = blocks.iter().find(|b| b.id == self.focus_block_id) {
                navigation::draw_focus_ring(fb, b.rect, Color::from_hex(0xFF6B6B));
            }
        }
    }

    /// Cycle to next app (Alt+Tab)
    pub fn cycle_focus(&mut self) {
        if self.apps.is_empty() {
            return;
        }
        self.focus_app = (self.focus_app + 1) % self.apps.len();
        // Reset focus block to first block in new app
        let blocks = self.apps[self.focus_app].focus_blocks();
        if !blocks.is_empty() {
            self.focus_block_id = blocks[0].id;
        }
    }

    /// Switch to specific app by index
    pub fn switch_to_app(&mut self, idx: usize) -> bool {
        if idx < self.apps.len() {
            self.focus_app = idx;
            let blocks = self.apps[self.focus_app].focus_blocks();
            if !blocks.is_empty() {
                self.focus_block_id = blocks[0].id;
            }
            true
        } else {
            false
        }
    }

    /// Handle mouse click - check if it's on a different app
    pub fn handle_mouse_click(&mut self, x: i32, y: i32) {
        for (idx, app) in self.apps.iter().enumerate() {
            let bounds = app.bounds();
            if x >= bounds.x && x < bounds.x + bounds.w as i32 &&
               y >= bounds.y && y < bounds.y + bounds.h as i32 {
                if idx != self.focus_app {
                    self.focus_app = idx;
                    let blocks = self.apps[self.focus_app].focus_blocks();
                    if !blocks.is_empty() {
                        self.focus_block_id = blocks[0].id;
                    }
                }
                break;
            }
        }
    }

    /// Get number of registered apps
    pub fn app_count(&self) -> usize {
        self.apps.len()
    }

    /// Get currently focused app index
    pub fn focused_app_index(&self) -> usize {
        self.focus_app
    }

    pub fn dispatch_event(
        &mut self,
        fb: &mut FramebufferWriter,
        theme: &Theme,
        event: AppEvent,
        accent: Color,
    ) {
        if self.apps.is_empty() {
            return;
        }
        
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
