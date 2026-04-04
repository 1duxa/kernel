//! # VM Module
//!
//! Clean public API for the kernel bytecode VM.
//!
//! This module intentionally presents a small, stable surface to the rest of
//! the kernel:
//!
//! - bytecode data structures from [`bytecode`]
//! - source parser from [`parser`]
//! - VM executor from [`runtime`]
//! - convenience helpers re-exported at the top level
//!
//! Most callers should only need:
//!
//! - [`Instruction`]
//! - [`Program`]
//! - [`parse_program`]
//! - [`compile_and_run`]
//! - [`example_program`]
//! - [`Vm`]
//! - [`VmError`]
//! - [`VmResult`]

pub mod bytecode;
pub mod parser;
pub mod runtime;
pub mod vm_process;

pub use bytecode::{example_program, example_program_advanced, Instruction, Program};
pub use parser::{parse_program, ParseError};
pub use runtime::{compile_and_run, Vm, VmError, VmResult};
pub use vm_process::{execute_program_in_process, VmProcess, allocate_vm_page, free_vm_page};
