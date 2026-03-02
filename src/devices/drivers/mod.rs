//! Device Drivers
//!
//! This module contains drivers for various hardware devices:
//! - PS/2 Keyboard (IRQ1)
//! - PS/2 Mouse (IRQ12)
pub mod ps2_keyboard;
pub mod ps2_mouse;

#[allow(unused)]
pub use ps2_keyboard::{dequeue_scancode, enqueue_scancode, KeyEvent, ScancodeDecoder};

#[allow(unused)]
pub use ps2_mouse::{enqueue_mouse_byte, init, poll_mouse_event, MouseDecoder, MouseEvent};
