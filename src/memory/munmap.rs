use crate::println;
use bootloader_api::info::MemoryRegionKind;
use bootloader_api::BootInfo;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use x86_64::registers::control::Cr3;
use x86_64::{
    structures::paging::{
        FrameAllocator, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

pub fn sys_munmap(
    addr: usize,
    length: usize,
) -> Result<usize, crate::syscalls::dispatcher::SyscallError> {
    use crate::syscalls::dispatcher::SyscallError;

    if length == 0 || addr & 0xFFF != 0 {
        return Err(SyscallError::InvalidArgument);
    }

    Ok(0)
}
