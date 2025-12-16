//! # Kernel Initialization Module
//!
//! Provides kernel bootstrap and status tracking infrastructure.
//!
//! ## Submodules
//!
//! - `init`: Kernel initialization sequence
//! - `status`: Component status tracking for startup display
//!
//! ## Status Tracking
//!
//! Components register themselves and update their initialization status.
//! This is used to display a boot splash showing initialization progress.
//!
//! ## Example
//!
//! ```ignore
//! use crate::core::kernel::{register_component, update_component_status, InitStatus};
//!
//! register_component("Memory", InitStatus::Pending);
//! // ... initialize memory ...
//! update_component_status("Memory", InitStatus::Done);
//! ```

/// Kernel initialization and bootstrap module
pub mod init;
pub mod status;

pub use init::init_kernel;
pub use status::{register_component, update_component_status, InitStatus};

