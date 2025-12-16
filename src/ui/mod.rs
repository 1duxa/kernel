//! # User Interface Module
//!
//! Provides theming and widget primitives for building graphical
//! user interfaces in the kernel.
//!
//! ## Modules
//!
//! - `theme`: Color themes (dark/light modern)
//! - `widgets`: Basic UI primitives (Panel, Label, VStack, HStack)
//!
//! ## Overview
//!
//! This module provides a lightweight widget system designed for
//! `no_std` environments. It uses `embedded_graphics` for text
//! rendering and custom primitives for shapes.
//!
//! ## Example
//!
//! ```ignore
//! use crate::ui::{Theme, widgets::*};
//!
//! let theme = Theme::dark_modern();
//! let mut panel = Panel::new(theme.surface);
//! panel.layout(Rect::new(0, 0, 200, 100));
//! panel.render(&mut fb, &theme);
//! ```

pub mod theme;
pub mod widgets;
pub use theme::Theme;

