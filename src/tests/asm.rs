use crate::memory::{mmap::sys_mmap, munmap::sys_munmap};
use crate::{log_error, log_info, println};
use alloc::alloc::{alloc, dealloc};
use alloc::{string::String, vec::Vec};
use core::alloc::Layout;

const MAX_CODE_SIZE: usize = 4096;
const PAGE_SIZE: usize = 4096;
const PROT_READ: usize = 0x1;
const PROT_WRITE: usize = 0x2;
const PROT_EXEC: usize = 0x4;

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

        match sys_mmap(0, map_size, PROT_READ | PROT_WRITE | PROT_EXEC, 0, 0, 0) {
            Ok(virt_addr) => {
                if virt_addr == 0 {
                    log_error!("ASM: mmap returned null address");
                    return Err(String::from("mmap returned null address"));
                }

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
                log_error!(
                    "ASM: mmap failed: {:?} — cannot execute on heap (NX bit)",
                    e
                );
                Err(String::from(
                    "mmap failed; heap execution blocked by NX bit",
                ))
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
    /// Returns 42: `mov eax, 42; ret`
    pub fn simple_return_42() -> &'static [u8] {
        &[0xb8, 0x2a, 0x00, 0x00, 0x00, 0xc3]
    }

    /// Returns 3: `mov eax, 1; mov ecx, 2; add eax, ecx; ret`
    pub fn simple_add_1_2() -> &'static [u8] {
        &[
            0xb8, 0x01, 0x00, 0x00, 0x00, // mov eax, 1
            0xb9, 0x02, 0x00, 0x00, 0x00, // mov ecx, 2
            0x01, 0xc8, // add eax, ecx
            0xc3, // ret
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
