//! # Applications Module
//!
//! Contains concrete application implementations for the kernel.
//!
//! ## Available Apps
//!
//! - `terminal_app`: Interactive terminal/shell application
//! - `logs_app`: Kernel log viewer application
//!
//! ## Architecture
//!
//! Each application implements the `App` trait from `crate::app`,
//! providing consistent handling for:
//! - Initialization
//! - Event processing (keyboard, mouse)
//! - Layout and rendering
//! - Focus management

pub mod logs_app;
pub mod terminal_app;