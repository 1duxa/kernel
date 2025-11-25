use bootloader_api::info::MemoryRegionKind;
use bootloader_api::BootInfo;
use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicU64, Ordering};
pub mod allocators;
use x86_64::{
    structures::paging::{
        Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
        Mapper, FrameAllocator, OffsetPageTable,
    },
    VirtAddr, PhysAddr,
};

const KERNEL_HEAP_SIZE: usize = 256 * 1024 * 1024;

#[repr(align(4096))]
struct HeapBuffer([u8; KERNEL_HEAP_SIZE]);
static mut KERNEL_HEAP_BUFFER: HeapBuffer = HeapBuffer([0; KERNEL_HEAP_SIZE]);

static PHYSICAL_MEMORY_OFFSET: AtomicU64 = AtomicU64::new(0);
static PHYSICAL_MEMORY_START: AtomicU64 = AtomicU64::new(0);
static PHYSICAL_MEMORY_END: AtomicU64 = AtomicU64::new(0);
static NEXT_PHYSICAL_FRAME: AtomicU64 = AtomicU64::new(0);


#[global_allocator]
static KERNEL_ALLOCATOR: LockedHeap = LockedHeap::new();

pub struct LockedHeap {
    inner: spin::Mutex<Option<FixedSizeBlockAllocator>>,
}

impl LockedHeap {
    pub const fn new() -> Self {
        Self {
            inner: spin::Mutex::new(None),
        }
    }
}

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut guard = self.inner.lock();
        if let Some(allocator) = guard.as_ref() {
            allocator.alloc(layout)
        } else {
            core::ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut guard = self.inner.lock();
        if let Some(allocator) = guard.as_ref() {
            allocator.dealloc(ptr, layout);
        }
    }
}

pub struct GlobalFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for GlobalFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        loop {
            let current = NEXT_PHYSICAL_FRAME.load(Ordering::Relaxed);
            let frame_addr = (current + 4095) & !4095;
            let next_frame = frame_addr + 4096;
            
            let memory_end = PHYSICAL_MEMORY_END.load(Ordering::Relaxed);
            if next_frame >= memory_end {
                return None;
            }
            
            match NEXT_PHYSICAL_FRAME.compare_exchange_weak(
                current,
                next_frame,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let phys_addr = PhysAddr::new(frame_addr);
                    return Some(PhysFrame::containing_address(phys_addr));
                }
                Err(_) => continue,
            }
        }
    }
}

pub fn allocate_frame() -> Option<PhysFrame<Size4KiB>> {
    GlobalFrameAllocator.allocate_frame()
}

pub unsafe fn active_page_table() -> OffsetPageTable<'static> {
    let (level_4_frame, _) = x86_64::registers::control::Cr3::read();
    let phys = level_4_frame.start_address();
    let offset = PHYSICAL_MEMORY_OFFSET.load(Ordering::Relaxed);
    let virt = VirtAddr::new(phys.as_u64() + offset);
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    
    OffsetPageTable::new(&mut *page_table_ptr, VirtAddr::new(offset))
}

pub unsafe fn init(boot_info: &BootInfo) -> Result<(), &'static str> {
    let phys_offset = boot_info.physical_memory_offset
        .into_option()
        .unwrap_or(0);
    PHYSICAL_MEMORY_OFFSET.store(phys_offset, Ordering::Release);
    
    let mut largest_region_start = 0u64;
    let mut largest_region_size = 0u64;
    
    for region in boot_info.memory_regions.iter() {
        if region.kind == MemoryRegionKind::Usable {
            let size = region.end - region.start;
            if size > largest_region_size {
                largest_region_start = region.start;
                largest_region_size = size;
            }
        }
    }
    
    if largest_region_size == 0 {
        return Err("No usable memory found");
    }
    
    let frame_start = largest_region_start.max(16 * 1024 * 1024);
    let frame_end = largest_region_start + largest_region_size;
    
    PHYSICAL_MEMORY_START.store(frame_start, Ordering::Release);
    PHYSICAL_MEMORY_END.store(frame_end, Ordering::Release);
    NEXT_PHYSICAL_FRAME.store(frame_start, Ordering::Release);
    
    let allocator = FixedSizeBlockAllocator::new();
    allocator.init(KERNEL_HEAP_BUFFER.0.as_ptr() as usize, KERNEL_HEAP_SIZE)
        .map_err(|_| "Failed to initialize kernel heap")?;
    *KERNEL_ALLOCATOR.inner.lock() = Some(allocator);
    
    Ok(())
}

pub unsafe fn create_process_page_table() -> Result<PhysFrame<Size4KiB>, &'static str> {
    let frame = allocate_frame().ok_or("Out of physical memory")?;
    
    let offset = PHYSICAL_MEMORY_OFFSET.load(Ordering::Relaxed);
    let virt_addr = VirtAddr::new(frame.start_address().as_u64() + offset);
    let table_ptr = virt_addr.as_mut_ptr::<PageTable>();
    core::ptr::write_bytes(table_ptr, 0, 1);
    
    let (current_p4_frame, _) = x86_64::registers::control::Cr3::read();
    let current_p4_virt = VirtAddr::new(current_p4_frame.start_address().as_u64() + offset);
    let current_p4: &PageTable = &*current_p4_virt.as_ptr();
    let new_p4: &mut PageTable = &mut *table_ptr;
    
    for i in 256..512 {
        new_p4[i] = current_p4[i].clone();
    }
    
    Ok(frame)
}

pub unsafe fn map_page_in_table(
    p4_frame: PhysFrame<Size4KiB>,
    page: Page<Size4KiB>,
    frame: PhysFrame<Size4KiB>,
    flags: PageTableFlags,
) -> Result<(), &'static str> {
    let offset = PHYSICAL_MEMORY_OFFSET.load(Ordering::Relaxed);
    let p4_virt = VirtAddr::new(p4_frame.start_address().as_u64() + offset);
    let p4_ptr: *mut PageTable = p4_virt.as_mut_ptr();
    
    let mut mapper = OffsetPageTable::new(&mut *p4_ptr, VirtAddr::new(offset));
    let mut frame_allocator = GlobalFrameAllocator;
    
    mapper.map_to(page, frame, flags, &mut frame_allocator)
        .map_err(|_| "Failed to map page")?
        .flush();
    
    Ok(())
}

pub fn sys_brk(addr: u64) -> Result<usize, crate::syscalls::dispatcher::SyscallError> {
    use crate::syscalls::dispatcher::SyscallError;
    
    const HEAP_START: u64 = 0x4444_4444_0000;
    const HEAP_SIZE: u64 = 100 * 1024 * 1024;
    static PROGRAM_BREAK: AtomicU64 = AtomicU64::new(HEAP_START);
    
    if addr == 0 {
        return Ok(PROGRAM_BREAK.load(Ordering::Relaxed) as usize);
    }
    
    let current_brk = PROGRAM_BREAK.load(Ordering::Relaxed);
    if addr < HEAP_START || addr > HEAP_START + HEAP_SIZE {
        return Ok(current_brk as usize);
    }
    
    if addr > current_brk {
        let start_page = Page::containing_address(VirtAddr::new(current_brk));
        let end_page = Page::containing_address(VirtAddr::new(addr - 1));
        
        let mut mapper = unsafe { active_page_table() };
        let mut frame_allocator = GlobalFrameAllocator;
        
        for page in Page::range_inclusive(start_page, end_page) {
            let frame = frame_allocator.allocate_frame()
                .ok_or(SyscallError::NoMemory)?;
            
            let flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;
            
            unsafe {
                mapper.map_to(page, frame, flags, &mut frame_allocator)
                    .map_err(|_| SyscallError::NoMemory)?
                    .flush();
            }
        }
    }
    
    PROGRAM_BREAK.store(addr, Ordering::Relaxed);
    Ok(addr as usize)
}

pub fn sys_mmap(
    addr: usize,
    length: usize,
    prot: usize,
    _flags: usize,
    _fd: i32,
    _offset: usize,
) -> Result<usize, crate::syscalls::dispatcher::SyscallError> {
    use crate::syscalls::dispatcher::SyscallError;
    
    if length == 0 {
        return Err(SyscallError::InvalidArgument);
    }
    
    static NEXT_MMAP_ADDR: AtomicU64 = AtomicU64::new(0x8000_0000_0000);
    
    let page_count = (length + 4095) / 4096;
    let actual_size = page_count * 4096;
    
    let virt_addr = if addr != 0 {
        addr as u64 & !0xFFF
    } else {
        NEXT_MMAP_ADDR.fetch_add(actual_size as u64, Ordering::SeqCst)
    };
    
    let mut mapper = unsafe { active_page_table() };
    let mut frame_allocator = GlobalFrameAllocator;
    
    let start_page = Page::containing_address(VirtAddr::new(virt_addr));
    let end_page = Page::containing_address(VirtAddr::new(virt_addr + actual_size as u64 - 1));
    
    let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
    if prot & 0x2 != 0 {
        flags |= PageTableFlags::WRITABLE;
    }
    if prot & 0x4 == 0 {
        flags |= PageTableFlags::NO_EXECUTE;
    }
    
    for page in Page::range_inclusive(start_page, end_page) {
        let frame = frame_allocator.allocate_frame()
            .ok_or(SyscallError::NoMemory)?;
        
        unsafe {
            mapper.map_to(page, frame, flags, &mut frame_allocator)
                .map_err(|_| SyscallError::NoMemory)?
                .flush();
            
            let page_ptr = page.start_address().as_u64() as *mut u8;
            core::ptr::write_bytes(page_ptr, 0, 4096);
        }
    }
    
    Ok(virt_addr as usize)
}

pub fn sys_munmap(addr: usize, length: usize) -> Result<usize, crate::syscalls::dispatcher::SyscallError> {
    use crate::syscalls::dispatcher::SyscallError;
    
    if length == 0 || addr & 0xFFF != 0 {
        return Err(SyscallError::InvalidArgument);
    }
    
    let page_count = (length + 4095) / 4096;
    let start_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(addr as u64));
    let end_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new((addr + page_count * 4096 - 1) as u64));
    
    let mut mapper = unsafe { active_page_table() };
    
    for page in Page::range_inclusive(start_page, end_page) {
        if let Ok((_, flush)) = mapper.unmap(page) {
            flush.flush();
        }
    }
    
    Ok(0)
}

use core::sync::atomic::AtomicUsize;

use crate::memory::allocators::FixedSizeBlockAllocator;

static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

pub unsafe fn sys_pstart(code_ptr: *const u8, code_size: usize) -> Result<usize, &'static str> {
    if code_ptr.is_null() || code_size == 0 || code_size > 10 * 1024 * 1024 {
        return Err("Invalid arguments");
    }
    
    let code = core::slice::from_raw_parts(code_ptr, code_size);
    
    let p4_frame = create_process_page_table()?;
    
    const CODE_START: u64 = 0x400000;
    let code_pages = (code_size + 4095) / 4096;
    
    for i in 0..code_pages {
        let page = Page::containing_address(VirtAddr::new(CODE_START + i as u64 * 4096));
        let frame = allocate_frame().ok_or("Out of memory")?;
        
        map_page_in_table(
            p4_frame,
            page,
            frame,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
        )?;
        
        let offset = PHYSICAL_MEMORY_OFFSET.load(Ordering::Relaxed);
        let dst = (frame.start_address().as_u64() + offset) as *mut u8;
        let src = &code[i * 4096..] as *const _ as *const u8;
        let copy_size = core::cmp::min(4096, code_size - i * 4096);
        core::ptr::copy_nonoverlapping(src, dst, copy_size);
    }
    
    let stack_page = Page::containing_address(VirtAddr::new(0xFFFF_F000_0000));
    let stack_frame = allocate_frame().ok_or("Out of memory")?;
    
    map_page_in_table(
        p4_frame,
        stack_page,
        stack_frame,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
    )?;
    
    let pid = NEXT_PID.fetch_add(1, Ordering::SeqCst) as usize;
    
    Ok(pid)
}

pub fn memory_stats(_boot_info: &BootInfo) {
    // Memory stats simplified for soft-float target compatibility
}
