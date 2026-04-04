//! System Call Interface

pub mod numbers;
pub mod dispatcher;
pub mod handlers;

pub use dispatcher::SyscallError;
