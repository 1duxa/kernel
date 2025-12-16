//! # Mouse Cursor
//!
//! Provides mouse cursor tracking and rendering.
//!
//! ## Features
//! - Tracks absolute mouse position from relative movements
//! - Renders cursor sprite with transparency
//! - Saves/restores background under cursor for flicker-free updates

use crate::devices::framebuffer::color::Color;
use crate::devices::framebuffer::framebuffer::FramebufferWriter;
use core::sync::atomic::{AtomicI32, AtomicBool, Ordering};

// =============================================================================
// CURSOR STATE
// =============================================================================

/// Global cursor position (atomic for interrupt-safe updates)
static CURSOR_X: AtomicI32 = AtomicI32::new(0);
static CURSOR_Y: AtomicI32 = AtomicI32::new(0);
static CURSOR_VISIBLE: AtomicBool = AtomicBool::new(true);
static CURSOR_NEEDS_REDRAW: AtomicBool = AtomicBool::new(true);

// Screen bounds for clamping
static SCREEN_WIDTH: AtomicI32 = AtomicI32::new(800);
static SCREEN_HEIGHT: AtomicI32 = AtomicI32::new(600);

// =============================================================================
// CURSOR BITMAP (12x19 arrow)
// =============================================================================

/// Cursor bitmap: 0 = transparent, 1 = black outline, 2 = white fill
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

/// Initialize cursor with screen dimensions
pub fn init(screen_width: usize, screen_height: usize) {
    SCREEN_WIDTH.store(screen_width as i32, Ordering::Relaxed);
    SCREEN_HEIGHT.store(screen_height as i32, Ordering::Relaxed);
    
    // Start cursor at center of screen
    CURSOR_X.store(screen_width as i32 / 2, Ordering::Relaxed);
    CURSOR_Y.store(screen_height as i32 / 2, Ordering::Relaxed);
    CURSOR_NEEDS_REDRAW.store(true, Ordering::Relaxed);
}

/// Update cursor position from relative mouse movement
pub fn update_position(dx: i16, dy: i16) {
    let old_x = CURSOR_X.load(Ordering::Relaxed);
    let old_y = CURSOR_Y.load(Ordering::Relaxed);
    
    let screen_w = SCREEN_WIDTH.load(Ordering::Relaxed);
    let screen_h = SCREEN_HEIGHT.load(Ordering::Relaxed);
    
    // Apply movement and clamp to screen bounds
    let new_x = (old_x + dx as i32).clamp(0, screen_w - 1);
    let new_y = (old_y + dy as i32).clamp(0, screen_h - 1);
    
    if new_x != old_x || new_y != old_y {
        CURSOR_X.store(new_x, Ordering::Relaxed);
        CURSOR_Y.store(new_y, Ordering::Relaxed);
        CURSOR_NEEDS_REDRAW.store(true, Ordering::Relaxed);
    }
}

/// Get current cursor position
pub fn get_position() -> (i32, i32) {
    (
        CURSOR_X.load(Ordering::Relaxed),
        CURSOR_Y.load(Ordering::Relaxed),
    )
}

/// Set cursor visibility
pub fn set_visible(visible: bool) {
    let was_visible = CURSOR_VISIBLE.swap(visible, Ordering::Relaxed);
    if was_visible != visible {
        CURSOR_NEEDS_REDRAW.store(true, Ordering::Relaxed);
    }
}

/// Check if cursor needs redraw
pub fn needs_redraw() -> bool {
    CURSOR_NEEDS_REDRAW.load(Ordering::Relaxed)
}

/// Mark cursor as redrawn
pub fn mark_drawn() {
    CURSOR_NEEDS_REDRAW.store(false, Ordering::Relaxed);
}

/// Draw cursor on framebuffer
/// 
/// This draws directly to the node buffer. Call this AFTER rendering
/// the main content but BEFORE calling render_frame().
pub fn draw(fb: &mut FramebufferWriter) {
    if !CURSOR_VISIBLE.load(Ordering::Relaxed) {
        return;
    }
    
    let cx = CURSOR_X.load(Ordering::Relaxed);
    let cy = CURSOR_Y.load(Ordering::Relaxed);
    
    let outline_color = Color::BLACK;
    let fill_color = Color::WHITE;
    
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
            
            let color = if pixel == 1 { outline_color } else { fill_color };
            fb.put_pixel(px as usize, py as usize, color);
        }
    }
}

/// Get cursor dimensions for save/restore operations
pub fn dimensions() -> (usize, usize) {
    (CURSOR_WIDTH, CURSOR_HEIGHT)
}
