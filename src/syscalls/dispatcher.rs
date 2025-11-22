use crate::syscall::numbers::SyscallNumber;
use crate::syscall::handlers;
use x86_64::structures::idt::InterruptStackFrame;

/// System call result type
pub type SyscallResult = Result<usize, SyscallError>;

/// System call errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallError {
    InvalidSyscall,
    InvalidArgument,
    PermissionDenied,
    NotImplemented,
    BadFileDescriptor,
    NoMemory,
    IoError,
}

impl SyscallError {
    pub fn as_errno(self) -> isize {
        match self {
            Self::InvalidSyscall => -1,
            Self::InvalidArgument => -22,  // EINVAL
            Self::PermissionDenied => -13, // EACCES
            Self::NotImplemented => -38,   // ENOSYS
            Self::BadFileDescriptor => -9, // EBADF
            Self::NoMemory => -12,         // ENOMEM
            Self::IoError => -5,           // EIO
        }
    }
}

/// System call context - contains all registers from the syscall
#[derive(Debug, Clone, Copy)]
pub struct SyscallContext {
    pub syscall_num: usize,
    pub arg0: usize,
    pub arg1: usize,
    pub arg2: usize,
    pub arg3: usize,
    pub arg4: usize,
    pub arg5: usize,
}
// TODO: Those registers are not set up yet
impl SyscallContext {
    /// Create from interrupt stack frame
    /// 
    /// x86_64 syscall convention (using `syscall` instruction):
    ///   rax = syscall number
    ///   rdi = arg0
    ///   rsi = arg1
    ///   rdx = arg2
    ///   r10 = arg3  (rcx is used for return address)
    ///   r8  = arg4
    ///   r9  = arg5
    pub fn from_registers(
        rax: usize,
        rdi: usize,
        rsi: usize,
        rdx: usize,
        r10: usize,
        r8: usize,
        r9: usize,
    ) -> Self {
        Self {
            syscall_num: rax,
            arg0: rdi,
            arg1: rsi,
            arg2: rdx,
            arg3: r10,
            arg4: r8,
            arg5: r9,
        }
    }
}

/// Main syscall dispatcher
pub fn dispatch_syscall(ctx: SyscallContext) -> SyscallResult {
    let syscall = SyscallNumber::from(ctx.syscall_num);
    
    // Log syscall for debugging (remove in production)
    #[cfg(debug_assertions)]
    crate::println!("SYSCALL: {:?}({}, {}, {}, {}, {}, {})",
        syscall, ctx.arg0, ctx.arg1, ctx.arg2, ctx.arg3, ctx.arg4, ctx.arg5);
    
    match syscall {
        // I/O Operations
        SyscallNumber::Read => handlers::io::sys_read(
            ctx.arg0 as i32,
            ctx.arg1 as *mut u8,
            ctx.arg2,
        ),
        SyscallNumber::Write => handlers::io::sys_write(
            ctx.arg0 as i32,
            ctx.arg1 as *const u8,
            ctx.arg2,
        ),
        SyscallNumber::Open => handlers::io::sys_open(
            ctx.arg0 as *const u8,
            ctx.arg1,
            ctx.arg2,
        ),
        SyscallNumber::Close => handlers::io::sys_close(ctx.arg0 as i32),
        
        // Process Management
        SyscallNumber::Exit => handlers::process::sys_exit(ctx.arg0 as i32),
        SyscallNumber::GetPid => handlers::process::sys_getpid(),
        SyscallNumber::Fork => handlers::process::sys_fork(),
        SyscallNumber::Exec => handlers::process::sys_exec(
            ctx.arg0 as *const u8,
            ctx.arg1 as *const *const u8,
        ),
        SyscallNumber::Wait => handlers::process::sys_wait(ctx.arg0 as *mut i32),
        
        // Memory Management
        SyscallNumber::Mmap => handlers::memory::sys_mmap(
            ctx.arg0,
            ctx.arg1,
            ctx.arg2,
            ctx.arg3,
            ctx.arg4 as i32,
            ctx.arg5,
        ),
        SyscallNumber::Munmap => handlers::memory::sys_munmap(ctx.arg0, ctx.arg1),
        SyscallNumber::Brk => handlers::memory::sys_brk(ctx.arg0),
        
        // Time
        SyscallNumber::Sleep => handlers::time::sys_sleep(ctx.arg0),
        SyscallNumber::GetTime => handlers::time::sys_gettime(),
        
        // Not yet implemented
        _ => Err(SyscallError::NotImplemented),
    }
}
