/// Kernel initialization and bootstrap module
pub mod init;
pub mod status;

pub use init::init_kernel;
pub use status::{register_component, update_component_status, InitStatus};

