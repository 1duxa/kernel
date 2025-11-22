//! System call numbers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum SyscallNumber {
    // I/O Operations (0-19)
    Read = 0,
    Write = 1,
    Open = 2,
    Close = 3,
    
    // Process Management (20-39)
    Exit = 20,
    Fork = 21,
    Exec = 22,
    Wait = 23,
    GetPid = 24,
    
    // Memory Management (40-59)
    Mmap = 40,
    Munmap = 41,
    Brk = 42,
    
    // Time (60-79)
    Sleep = 60,
    GetTime = 61,
    
    // Signals (80-99)
    Kill = 80,
    Signal = 81,
    
    // File System (100-119)
    Chdir = 100,
    Mkdir = 101,
    
    // Unknown
    Unknown = usize::MAX,
}
impl From<usize> for SyscallNumber {
    fn from(num: usize) -> Self {
        match num {
            0 => Self::Read,
            1 => Self::Write,
            2 => Self::Open,
            3 => Self::Close,
            20 => Self::Exit,
            21 => Self::Fork,
            22 => Self::Exec,
            23 => Self::Wait,
            24 => Self::GetPid,
            40 => Self::Mmap,
            41 => Self::Munmap,
            42 => Self::Brk,
            60 => Self::Sleep,
            61 => Self::GetTime,
            80 => Self::Kill,
            81 => Self::Signal,
            100 => Self::Chdir,
            101 => Self::Mkdir,
            _ => Self::Unknown,
        }
    }
}