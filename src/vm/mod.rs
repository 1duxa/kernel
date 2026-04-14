//! # VM Module
//!
pub mod bytecode;
pub mod parser;
pub mod runtime;
pub mod vm_process;

pub use bytecode::{example_program, example_program_advanced, Instruction, Program};
pub use parser::{parse_program, ParseError};
pub use runtime::{compile_and_run, Vm, VmError, VmResult};
pub use vm_process::{allocate_vm_page, execute_program_in_process, free_vm_page, VmProcess};
