//! Device Subsystem
//!
//! Hardware device drivers and abstractions:
//! - `drivers`: PS/2 keyboard and mouse drivers
//! - `framebuffer`: Graphics output via linear framebuffer
//! - `mouse_cursor`: Mouse cursor rendering and tracking

pub mod drivers;
pub mod framebuffer;
pub mod mouse_cursor;
