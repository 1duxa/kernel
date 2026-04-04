use super::bytecode::Program;
use super::runtime::{Vm, VmError, VmResult};
use core::sync::atomic::{AtomicUsize, Ordering};

static NEXT_VM_PID: AtomicUsize = AtomicUsize::new(1000);

const VM_ARENA_PAGES: usize = 5;
const VM_ARENA_SIZE: usize = VM_ARENA_PAGES * 4096;

const PROT_READ: usize = 0x1;
const PROT_WRITE: usize = 0x2;

#[derive(Clone, Debug)]
pub struct VmProcessState {
    pub pid: usize,
    pub arena_addr: usize,
    pub arena_size: usize,
}

pub struct VmProcess {
    state: VmProcessState,
    vm_ptr: *mut Vm,
}

// PID PYTANNYAM
unsafe impl Send for VmProcess {}

impl VmProcess {
    pub fn create() -> Result<Self, VmError> {
        let pid = NEXT_VM_PID.fetch_add(1, Ordering::Relaxed);

        let arena_addr =
            crate::memory::mmap::sys_mmap(0, VM_ARENA_SIZE, PROT_READ | PROT_WRITE, 0, 0, 0)
                .map_err(|_| VmError::runtime("VmProcess: sys_mmap failed"))?;

        if arena_addr == 0 {
            return Err(VmError::runtime("VmProcess: sys_mmap returned null"));
        }

        crate::log_info!("VmProcess::create() - arena at {:#x}", arena_addr);

        if VM_ARENA_SIZE < core::mem::size_of::<Vm>() {
            return Err(VmError::runtime("VmProcess: arena too small for Vm struct"));
        }

        let vm_ptr = arena_addr as *mut Vm;

        unsafe {
            crate::log_info!("Initializing Vm at {:#x}...", arena_addr);
            vm_ptr.write(Vm::new());
            crate::log_info!("Vm::new() succeeded");
        }

        crate::log_info!(
            "VM[{}]: arena ready at {:#x} ({} bytes)",
            pid,
            arena_addr,
            VM_ARENA_SIZE
        );

        Ok(Self {
            state: VmProcessState {
                pid,
                arena_addr,
                arena_size: VM_ARENA_SIZE,
            },
            vm_ptr,
        })
    }

    pub fn pid(&self) -> usize {
        self.state.pid
    }
    pub fn memory_layout(&self) -> &VmProcessState {
        &self.state
    }

    /// EXEC IN ARENA
    pub fn execute_program(&mut self, program: &Program) -> Result<VmResult, VmError> {
        crate::log_info!(
            "VM[{}]: running {} instructions (arena {:#x})",
            self.state.pid,
            program.instructions.len(),
            self.state.arena_addr,
        );

        let vm = unsafe { &mut *self.vm_ptr };

        match vm.execute(program) {
            Ok(result) => {
                crate::log_info!(
                    "VM[{}]: done — {} steps, {} output bytes",
                    self.state.pid,
                    result.steps,
                    result.output_len,
                );
                Ok(result)
            }
            Err(e) => {
                crate::log_info!("VM[{}]: error — {}", self.state.pid, e.message());
                Err(e)
            }
        }
    }

    pub fn allocate_memory(&self, size: usize) -> Result<usize, VmError> {
        if size == 0 {
            return Err(VmError::runtime("cannot allocate 0 bytes"));
        }
        Ok(self.state.arena_addr + self.state.arena_size)
    }

    pub fn handle_syscall(&self, syscall_num: usize, args: &[usize]) -> Result<usize, VmError> {
        match syscall_num {
            1 => {
                // write(fd, buf, count) — just return count for now
                let count = if args.len() >= 3 { args[2] } else { 0 };
                crate::log_info!("VM[{}]: syscall write count={}", self.state.pid, count);
                Ok(count)
            }
            4 => {
                // exit(code)
                let code = args.first().copied().unwrap_or(0);
                crate::log_info!("VM[{}]: syscall exit code={}", self.state.pid, code);
                Ok(code)
            }
            _ => Err(VmError::runtime("unsupported syscall")),
        }
    }
}

impl Drop for VmProcess {
    fn drop(&mut self) {
        unsafe {
            core::ptr::drop_in_place(self.vm_ptr);
        }

        let _ = crate::memory::munmap::sys_munmap(self.state.arena_addr, self.state.arena_size);

        crate::log_info!(
            "VM[{}]: arena {:#x} freed",
            self.state.pid,
            self.state.arena_addr
        );
    }
}

/// MAIN
pub fn execute_program_in_process(source: &str) -> Result<VmResult, VmError> {
    crate::log_info!("VM: parsing source ({} bytes)", source.len());

    let program =
        crate::vm::parser::parse_program(source).map_err(|_| VmError::parse("parse failed"))?;

    crate::log_info!("VM: parsed — {} instructions", program.instructions.len());

    let mut proc = VmProcess::create()?;
    proc.execute_program(&program)
}
/// YUSAYE KERNEL MEM
pub fn execute_simple(source: &str) -> Result<VmResult, VmError> {
    let program =
        crate::vm::parser::parse_program(source).map_err(|_| VmError::parse("parse failed"))?;
    let mut vm = Vm::new();
    vm.execute(&program)
}

pub fn allocate_vm_page() -> Result<usize, VmError> {
    crate::memory::mmap::sys_mmap(0, 4096, PROT_READ | PROT_WRITE, 0, 0, 0)
        .map_err(|_| VmError::runtime("allocate_vm_page: sys_mmap failed"))
}

pub fn free_vm_page(addr: usize) -> Result<(), VmError> {
    crate::memory::munmap::sys_munmap(addr, 4096)
        .map(|_| ())
        .map_err(|_| VmError::runtime("free_vm_page: sys_munmap failed"))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_process_creation() {
        let proc = VmProcess::create();
        assert!(proc.is_ok());
        let p = proc.unwrap();
        assert!(p.pid() >= 1000);
        assert!(p.state.arena_addr != 0);
        assert_eq!(p.state.arena_size, VM_ARENA_SIZE);
        // Drop here calls sys_munmap automatically
    }

    #[test]
    fn test_vm_arena_size_fits_vm() {
        assert!(
            VM_ARENA_SIZE >= core::mem::size_of::<Vm>(),
            "arena ({} B) too small for Vm ({} B)",
            VM_ARENA_SIZE,
            core::mem::size_of::<Vm>()
        );
    }
}
