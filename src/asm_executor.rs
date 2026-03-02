//! # JIT Assembly Executor
//!
//! Executes dynamically generated x86_64 machine code by allocating
//! executable memory via `sys_mmap` and running the provided bytecode.
//!
//! ## Safety
//!
//! Executing arbitrary bytecode is inherently unsafe and can crash the kernel.
//! The caller must ensure the bytecode is valid x86_64 code that returns via `ret`.
//!
//! ## Memory Management
//!
//! - Uses `sys_mmap` to allocate RWX pages for code execution
//! - Falls back to heap allocation if mmap fails (may fail due to NX bit)
//! - Memory is automatically freed after execution

use crate::memory::{sys_mmap, sys_munmap};
use crate::{log_error, log_info, println};
use alloc::alloc::{alloc, dealloc};
use alloc::{string::String, vec::Vec};
use core::alloc::Layout;

const MAX_CODE_SIZE: usize = 4096;
const PAGE_SIZE: usize = 4096;
const PROT_WRITE: usize = 0x2;
const PROT_EXEC: usize = 0x4;

/// JIT code executor for running dynamically generated machine code. UNSAFE!!!
pub struct AsmExecutor;

impl AsmExecutor {
    pub fn execute(code: &[u8]) -> Result<u64, String> {
        if code.is_empty() {
            log_error!("ASM: Empty code provided");
            return Err(String::from("Empty code"));
        }

        if code.len() > MAX_CODE_SIZE {
            log_error!("ASM: Code too large ({} bytes)", code.len());
            return Err(String::from("Code too large (max 4096 bytes)"));
        }

        let map_size = ((code.len() + PAGE_SIZE - 1) / PAGE_SIZE) * PAGE_SIZE;
        log_info!("ASM: Executing {} bytes", code.len());

        match sys_mmap(0, map_size, PROT_WRITE | PROT_EXEC, 0, 0, 0) {
            Ok(virt_addr) => {
                println!("ASM_EXECUTOR: mmap {:#x}", virt_addr);
                let result = unsafe {
                    let dst = virt_addr as *mut u8;
                    core::ptr::copy_nonoverlapping(code.as_ptr(), dst, code.len());
                    execute_code(dst as *const ())
                };
                let _ = sys_munmap(virt_addr, map_size);
                log_info!("ASM: Result = {}", result);
                Ok(result)
            }
            Err(e) => {
                println!("ASM_EXECUTOR: mmap failed, heap fallback: {:?}", e);
                Self::execute_on_heap(code)
            }
        }
    }

    fn execute_on_heap(code: &[u8]) -> Result<u64, String> {
        unsafe {
            let layout = Layout::from_size_align_unchecked(code.len(), 16);
            let code_ptr = alloc(layout);

            if code_ptr.is_null() {
                log_error!("ASM: Heap allocation failed");
                return Err(String::from("Failed to allocate code memory"));
            }

            core::ptr::copy_nonoverlapping(code.as_ptr(), code_ptr, code.len());
            let result = execute_code(code_ptr as *const ());
            dealloc(code_ptr, layout);

            log_info!("ASM: Heap result = {}", result);
            Ok(result)
        }
    }
}

extern "C" fn execute_code(code_ptr: *const ()) -> u64 {
    unsafe {
        let code_fn: extern "C" fn() -> u64 = core::mem::transmute(code_ptr);
        code_fn()
    }
}

/// Pre-built assembly programs for testing.
pub struct AsmProgram;

impl AsmProgram {
    /// Returns 42: `mov eax, 42; ret`
    pub fn simple_return_42() -> &'static [u8] {
        &[0xb8, 0x2a, 0x00, 0x00, 0x00, 0xc3]
    }

    /// Returns 3: `mov eax, 1; mov ebx, 2; add eax, ebx; ret`
    pub fn simple_add_1_2() -> &'static [u8] {
        &[
            0xb8, 0x01, 0x00, 0x00, 0x00, 0xbb, 0x02, 0x00, 0x00, 0x00, 0x01, 0xd8, 0xc3,
        ]
    }

    /// Build code that returns a specific 64-bit value.
    pub fn return_argument(value: u64) -> Vec<u8> {
        let mut code = Vec::new();
        code.push(0x48);
        code.push(0xb8);
        code.extend_from_slice(&value.to_le_bytes());
        code.push(0xc3);
        code
    }
}
