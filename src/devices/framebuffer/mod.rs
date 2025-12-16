//! # Framebuffer Graphics Module
//!
//! Provides framebuffer access and graphics rendering capabilities.
//!
//! ## Modules
//!
//! - `framebuffer`: Main `FramebufferWriter` with tiled rendering
//! - `color`: `Color` type with common color constants
//!
//! ## Architecture
//!
//! The framebuffer uses a tiled renderer for efficient updates:
//! 1. Drawing operations modify a node buffer (not real framebuffer)
//! 2. Tiles are marked dirty when modified
//! 3. `render_frame()` only copies dirty tiles to real framebuffer
//!
//! This reduces memory bandwidth and improves performance for
//! partial screen updates (common in UI rendering).
//!
//! ## Tile System
//!
//! - Tiles are 32x32 pixels
//! - Per-row hashing detects actual changes within tiles
//! - Only truly modified rows are copied to framebuffer

pub mod framebuffer;
pub mod color;