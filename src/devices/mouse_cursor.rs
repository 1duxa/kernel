//! # Mouse Cursor
//!
//! Provides mouse cursor tracking and rendering.

use crate::{
    devices::framebuffer::framebuffer::FramebufferWriter, println, ui_provider::color::Color,
};
use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use alloc::vec::Vec;

// =============================================================================
// CURSOR STATE
// =============================================================================

static CURSOR_X: AtomicI32 = AtomicI32::new(0);
static CURSOR_Y: AtomicI32 = AtomicI32::new(0);
static CURSOR_VISIBLE: AtomicBool = AtomicBool::new(true);
static CURSOR_NEEDS_REDRAW: AtomicBool = AtomicBool::new(true);

static mut SAVED_BACKGROUND: Option<(i32, i32, Vec<Color>)> = None;

// Screen bounds
static SCREEN_WIDTH: AtomicI32 = AtomicI32::new(800);
static SCREEN_HEIGHT: AtomicI32 = AtomicI32::new(600);

// =============================================================================
// CURSOR BITMAP (12x19 arrow)
// =============================================================================

const CURSOR_WIDTH: usize = 12;
const CURSOR_HEIGHT: usize = 19;

#[rustfmt::skip]
const CURSOR_BITMAP: [[u8; CURSOR_WIDTH]; CURSOR_HEIGHT] = [
    [1,0,0,0,0,0,0,0,0,0,0,0],
    [1,1,0,0,0,0,0,0,0,0,0,0],
    [1,2,1,0,0,0,0,0,0,0,0,0],
    [1,2,2,1,0,0,0,0,0,0,0,0],
    [1,2,2,2,1,0,0,0,0,0,0,0],
    [1,2,2,2,2,1,0,0,0,0,0,0],
    [1,2,2,2,2,2,1,0,0,0,0,0],
    [1,2,2,2,2,2,2,1,0,0,0,0],
    [1,2,2,2,2,2,2,2,1,0,0,0],
    [1,2,2,2,2,2,2,2,2,1,0,0],
    [1,2,2,2,2,2,2,2,2,2,1,0],
    [1,2,2,2,2,2,2,1,1,1,1,1],
    [1,2,2,2,1,2,2,1,0,0,0,0],
    [1,2,2,1,0,1,2,2,1,0,0,0],
    [1,2,1,0,0,1,2,2,1,0,0,0],
    [1,1,0,0,0,0,1,2,2,1,0,0],
    [1,0,0,0,0,0,1,2,2,1,0,0],
    [0,0,0,0,0,0,0,1,2,2,1,0],
    [0,0,0,0,0,0,0,1,1,1,1,0],
];

// =============================================================================
// PUBLIC API
// =============================================================================

pub fn init(screen_width: usize, screen_height: usize) {
    SCREEN_WIDTH.store(screen_width as i32, Ordering::Relaxed);
    SCREEN_HEIGHT.store(screen_height as i32, Ordering::Relaxed);

    CURSOR_X.store(screen_width as i32 / 2, Ordering::Relaxed);
    CURSOR_Y.store(screen_height as i32 / 2, Ordering::Relaxed);
    CURSOR_NEEDS_REDRAW.store(true, Ordering::Relaxed);
}

pub fn update_position(dx: i16, dy: i16) {
    let old_x = CURSOR_X.load(Ordering::Relaxed);
    let old_y = CURSOR_Y.load(Ordering::Relaxed);

    let screen_w = SCREEN_WIDTH.load(Ordering::Relaxed);
    let screen_h = SCREEN_HEIGHT.load(Ordering::Relaxed);

    let new_x = (old_x + dx as i32).clamp(0, screen_w - 1);
    let new_y = (old_y - dy as i32).clamp(0, screen_h - 1);

    if new_x != old_x || new_y != old_y {
        CURSOR_X.store(new_x, Ordering::Relaxed);
        CURSOR_Y.store(new_y, Ordering::Relaxed);
        CURSOR_NEEDS_REDRAW.store(true, Ordering::Relaxed);
    }
}

pub fn get_position() -> (i32, i32) {
    (
        CURSOR_X.load(Ordering::Relaxed),
        CURSOR_Y.load(Ordering::Relaxed),
    )
}

pub fn set_visible(visible: bool) {
    let was_visible = CURSOR_VISIBLE.swap(visible, Ordering::Relaxed);
    if was_visible != visible {
        CURSOR_NEEDS_REDRAW.store(true, Ordering::Relaxed);
    }
}

pub fn needs_redraw() -> bool {
    CURSOR_NEEDS_REDRAW.load(Ordering::Relaxed)
}

pub fn mark_drawn() {
    CURSOR_NEEDS_REDRAW.store(false, Ordering::Relaxed);
}


pub fn draw(fb: &mut FramebufferWriter) {
    unsafe {
        if let Some((old_x, old_y, ref pixels)) = SAVED_BACKGROUND {
            let mut pixel_idx = 0;
            for (row, bitmap_row) in CURSOR_BITMAP.iter().enumerate() {
                let py = old_y + row as i32;
                if py < 0 || py >= fb.height as i32 {
                    continue;
                }

                for (col, &pixel_mask) in bitmap_row.iter().enumerate() {
                    if pixel_mask == 0 {
                        continue; // Transparent - skip
                    }

                    let px = old_x + col as i32;
                    if px < 0 || px >= fb.width as i32 {
                        continue;
                    }

                    if pixel_idx < pixels.len() {
                        fb.put_pixel(px as usize, py as usize, pixels[pixel_idx]);
                        pixel_idx += 1;
                    }
                }
            }
        }

        if !CURSOR_VISIBLE.load(Ordering::Relaxed) {
            SAVED_BACKGROUND = None;
            return;
        }

        let cx = CURSOR_X.load(Ordering::Relaxed);
        let cy = CURSOR_Y.load(Ordering::Relaxed);
        let mut saved_pixels = Vec::new();

        for (row, bitmap_row) in CURSOR_BITMAP.iter().enumerate() {
            let py = cy + row as i32;
            if py < 0 || py >= fb.height as i32 {
                continue;
            }

            for (col, &pixel) in bitmap_row.iter().enumerate() {
                if pixel == 0 {
                    continue; // Transparent
                }

                let px = cx + col as i32;
                if px < 0 || px >= fb.width as i32 {
                    continue;
                }

                // Save the pixel that was there before
                let existing = fb.get_pixel(px as usize, py as usize);
                saved_pixels.push(existing);
            }
        }

        SAVED_BACKGROUND = Some((cx, cy, saved_pixels));

        // Draw cursor at new position
        let outline_color = Color::BLACK;
        let fill_color = Color::WHITE;

        for (row, bitmap_row) in CURSOR_BITMAP.iter().enumerate() {
            let py = cy + row as i32;
            if py < 0 || py >= fb.height as i32 {
                continue;
            }

            for (col, &pixel) in bitmap_row.iter().enumerate() {
                if pixel == 0 {
                    continue;
                }

                let px = cx + col as i32;
                if px < 0 || px >= fb.width as i32 {
                    continue;
                }

                let color = if pixel == 1 {
                    outline_color
                } else {
                    fill_color
                };
                fb.put_pixel(px as usize, py as usize, color);
            }
        }
    }
}

pub fn dimensions() -> (usize, usize) {
    (CURSOR_WIDTH, CURSOR_HEIGHT)
}
