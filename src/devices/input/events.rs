//! # Input Event Types
//!
//! Defines unified input event types for the kernel.
//!
//! ## Event Types
//!
//! - `InputEvent::Keyboard(KeyEvent)`: Keyboard press/release
//! - `InputEvent::Mouse(MouseEvent)`: Mouse movement/button
//!
//! ## InputEventHandler Trait
//!
//! Applications implement this trait to receive input events:
//!
//! ```ignore
//! impl InputEventHandler for MyApp {
//!     fn handle_keyboard(&mut self, event: KeyEvent) {
//!         // Handle key press
//!     }
//!     fn handle_mouse(&mut self, event: MouseEvent) {
//!         // Handle mouse movement
//!     }
//! }
//! ```
//!
//! Events can be dispatched via `InputEvent::dispatch()`.

use crate::devices::drivers::{KeyEvent, MouseEvent};
/// Unified input event type
#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    Keyboard(KeyEvent),
    Mouse(MouseEvent),
}

/// Input event handler trait
pub trait InputEventHandler {
    fn handle_keyboard(&mut self, event: KeyEvent);
    fn handle_mouse(&mut self, event: MouseEvent);
}

impl InputEvent {
    pub fn dispatch<E: InputEventHandler>(self, handler: &mut E) {
        match self {
            InputEvent::Keyboard(ke) => handler.handle_keyboard(ke),
            InputEvent::Mouse(me) => handler.handle_mouse(me),
        }
    }
}

