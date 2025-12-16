//! Device Subsystem
//!
//! Hardware device drivers and abstractions:
//! - `drivers`: PS/2 keyboard and mouse drivers
//! - `framebuffer`: Graphics output via linear framebuffer
//! - `input`: Input event types and handling
//! - `mouse_cursor`: Mouse cursor rendering and tracking

pub mod drivers;
pub mod framebuffer;
pub mod input;
pub mod mouse_cursor;
