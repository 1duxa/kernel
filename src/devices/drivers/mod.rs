//! Device Drivers
//!
//! This module contains drivers for various hardware devices:
//! - PS/2 Keyboard (IRQ1)
//! - PS/2 Mouse (IRQ12)

pub mod ps2_keyboard;
pub mod ps2_mouse;

pub use ps2_keyboard::{KeyEvent, ScancodeDecoder, enqueue_scancode, dequeue_scancode};
pub use ps2_mouse::{MouseEvent, MouseDecoder, poll_mouse_event, enqueue_mouse_byte};
