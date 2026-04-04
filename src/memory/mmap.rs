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

use crate::memory::allocators::block::FixedSizeBlockAllocator;
pub fn sys_mmap(
    addr: usize,
    length: usize,
    prot: usize,
    _flags: usize,
    _fd: i32,
    _offset: usize,
) -> Result<usize, crate::syscalls::dispatcher::SyscallError> {
    use crate::syscalls::dispatcher::SyscallError;
    println!("sys_mmap: requested {} bytes, flags={}", length, prot);
    if length == 0 {
        return Err(SyscallError::InvalidArgument);
    }

    if prot == 0 {
        return Err(SyscallError::InvalidArgument);
    }

    let page_count = (length + 4095) / 4096;
    let actual_size = page_count * 4096;

    let virt_addr = if addr != 0 {
        addr as u64 & !0xFFF
    } else {
        crate::memory::NEXT_MMAP_ADDR.fetch_add(actual_size as u64, Ordering::SeqCst)
    };
    println!("sys_mmap: returning virt = {:#x}", virt_addr);
    let mut flags = PageTableFlags::PRESENT;

    // PROT_WRITE (0x2)
    if prot & 0x2 != 0 {
        flags |= PageTableFlags::WRITABLE;
    }
    // PROT_EXEC (0x4) -  as no-execute
    if prot & 0x4 == 0 {
        flags |= PageTableFlags::NO_EXECUTE;
    }
    for i in 0..page_count {
        let page_virt = VirtAddr::new(virt_addr + (i * 4096) as u64);
        println!(
            "  mapped virt {:#x}   flags={:?}",
            page_virt.as_u64(),
            flags
        );
        let frame = crate::memory::allocate_frame().ok_or(SyscallError::NoMemory)?;
        crate::memory::zero_frame(frame);
        crate::memory::map_single_page(page_virt, frame, flags)
            .map_err(|_| SyscallError::NoMemory)?;
    }

    Ok(virt_addr as usize)
}
