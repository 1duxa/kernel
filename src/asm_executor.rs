//! JIT Code Executor
//!
//! This module allows executing dynamically generated machine code.
//! It uses `sys_mmap` to allocate executable memory and copies
//! the provided bytecode there before execution.
//!
//! # Safety
//! This is inherently unsafe - executing arbitrary bytecode can crash
//! the kernel or cause undefined behavior. Use with caution.
//!
//! # Example
//! ```ignore
//! use crate::asm_executor::{AsmExecutor, AsmProgram};
//!
//! // Execute code that returns 42
//! let result = AsmExecutor::execute(AsmProgram::simple_return_42());
//! assert_eq!(result, Ok(42));
//! ```

use crate::data_structures::vec::{String, Vec};
use crate::println;
use alloc::alloc::{alloc, dealloc};
use core::alloc::Layout;
use crate::memory::{sys_mmap, sys_munmap};

pub struct AsmExecutor;

impl AsmExecutor {
    pub fn execute(code: &[u8]) -> Result<u64, String> {
        if code.is_empty() {
            return Err(String::from("Empty code"));
        }

        if code.len() > 4096 {
            return Err(String::from("Code too large (max 4096 bytes)"));
        }

        const PROT_WRITE: usize = 0x2;
        const PROT_EXEC: usize = 0x4;

        // Round up to page size (4096 bytes)
        let map_size = ((code.len() + 4095) / 4096) * 4096;

        println!("ASM_EXECUTOR: attempting mmap size=0x{:x}", map_size);
        match sys_mmap(0, map_size as usize, PROT_WRITE | PROT_EXEC, 0, 0, 0) {
            Ok(virt_addr) => {
                println!("ASM_EXECUTOR: sys_mmap returned virt {:#x}", virt_addr);
                unsafe {
                    let dst = virt_addr as *mut u8;
                    println!("ASM_EXECUTOR: copying {} bytes to dst={:#x}", code.len(), dst as usize);
                    core::ptr::copy_nonoverlapping(code.as_ptr(), dst, code.len());
                    println!("ASM_EXECUTOR: copy to mmapped virt done, executing now at {:#x}", dst as usize);
                    let result = execute_code(dst as *const ());
                    println!("ASM_EXECUTOR: executed at dst={:#x} returned {:#x}", dst as usize, result);

                    // Unmap the mapped pages
                    let _ = sys_munmap(virt_addr, map_size as usize);

                    Ok(result)
                }
            }
            Err(e) => {
                println!("ASM_EXECUTOR: sys_mmap failed -> falling back to heap alloc: {:?}", e);
                // Fallback - allocate on heap and execute (may fail due to NX)
                unsafe {
                    let layout = Layout::from_size_align_unchecked(code.len(), 16);
                    let code_ptr = alloc(layout);

                    if code_ptr.is_null() {
                        return Err(String::from("Failed to allocate code memory"));
                    }

                    core::ptr::copy_nonoverlapping(code.as_ptr(), code_ptr, code.len());

                    println!("ASM_EXECUTOR: copying {} bytes to heap ptr={:#x}", code.len(), code_ptr as usize);
                    let result = execute_code(code_ptr as *const ());
                    println!("ASM_EXECUTOR: executed at heap ptr={:#x} returned {:#x}", code_ptr as usize, result);

                    dealloc(code_ptr, layout);

                    Ok(result)
                }
            }
        }
    }
}
extern "C" fn execute_code(code_ptr: *const ()) -> u64 {
    unsafe {
        let code_fn: extern "C" fn() -> u64 = core::mem::transmute(code_ptr);
        code_fn()
    }
}

pub struct AsmProgram;

impl AsmProgram {
    pub fn simple_return_42() -> &'static [u8] {
        &[
            0xb8, 0x2a, 0x00, 0x00, 0x00,
            0xc3,
        ]
    }

    pub fn simple_add_1_2() -> &'static [u8] {
        &[
            0xb8, 0x01, 0x00, 0x00, 0x00,
            0xbb, 0x02, 0x00, 0x00, 0x00,
            0x01, 0xd8,
            0xc3,
        ]
    }

    pub fn return_argument(value: u64) -> Vec<u8> {
        let mut code = Vec::new();
        code.push(0x48);
        code.push(0xb8);
        code.extend_from_slice(&value.to_le_bytes());
        code.push(0xc3);
        code
    }
}

