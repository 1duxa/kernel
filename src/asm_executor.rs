use crate::data_structures::vec::{String, Vec};
use alloc::alloc::{alloc, dealloc};
use core::alloc::Layout;

pub struct AsmExecutor;

impl AsmExecutor {
    pub fn execute(code: &[u8]) -> Result<u64, String> {
        if code.is_empty() {
            return Err(String::from("Empty code"));
        }

        if code.len() > 4096 {
            return Err(String::from("Code too large (max 4096 bytes)"));
        }

        unsafe {
            let layout = Layout::from_size_align_unchecked(code.len(), 16);
            let code_ptr = alloc(layout);
            
            if code_ptr.is_null() {
                return Err(String::from("Failed to allocate code memory"));
            }

            core::ptr::copy_nonoverlapping(code.as_ptr(), code_ptr, code.len());

            let result = execute_code(code_ptr as *const ());

            dealloc(code_ptr, layout);

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

pub use AsmProgram as Programs;
