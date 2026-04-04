use crate::memory::allocators::block::FixedSizeBlockAllocator;
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

pub fn sys_brk(addr: u64) -> Result<usize, crate::syscalls::dispatcher::SyscallError> {
    use crate::syscalls::dispatcher::SyscallError;

    const HEAP_START: u64 = 0x4444_4444_0000;
    static PROGRAM_BREAK: AtomicU64 = AtomicU64::new(HEAP_START);

    if addr == 0 {
        return Ok(PROGRAM_BREAK.load(Ordering::Relaxed) as usize);
    }

    let old_brk = PROGRAM_BREAK.load(Ordering::Relaxed);

    // Only map if growing
    if addr > old_brk {
        let start_page = (old_brk + 4095) & !4095; // align up
        let end_page = (addr + 4095) & !4095;

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;

        let mut virt = start_page;
        while virt < end_page {
            let page_virt = VirtAddr::new(virt);
            // Skip already-mapped pages
            if !crate::memory::page_is_mapped(page_virt) {
                let frame = crate::memory::allocate_frame().ok_or(SyscallError::NoMemory)?;
                crate::memory::zero_frame(frame);
                crate::memory::map_single_page(page_virt, frame, flags)
                    .map_err(|_| SyscallError::NoMemory)?;
            }
            virt += 4096;
        }
    }

    PROGRAM_BREAK.store(addr, Ordering::Relaxed);
    Ok(addr as usize)
}
