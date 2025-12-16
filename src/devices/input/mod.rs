//! # Input Device Module
//!
//! Provides unified input event handling for keyboard and mouse.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────┐    ┌──────────────┐
//! │   Keyboard   │    │    Mouse     │
//! │   Driver     │    │   Driver     │
//! └──────┬───────┘    └──────┬───────┘
//!        │                   │
//!        ▼                   ▼
//! ┌──────────────────────────────────┐
//! │          InputEvent              │
//! │  (Keyboard(KeyEvent) | Mouse)    │
//! └──────────────┬───────────────────┘
//!                │
//!                ▼
//! ┌──────────────────────────────────┐
//! │       InputEventHandler          │
//! │    (trait for applications)      │
//! └──────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! impl InputEventHandler for MyApp {
//!     fn handle_keyboard(&mut self, event: KeyEvent) { ... }
//!     fn handle_mouse(&mut self, event: MouseEvent) { ... }
//! }
//! ```

/// Unified input event system
pub mod events;

pub use events::*;