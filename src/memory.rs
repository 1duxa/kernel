use bootloader_api::info::MemoryRegionKind;
use bootloader_api::BootInfo;
use core::{alloc::{GlobalAlloc, Layout}, slice::SlicePattern};
use spin::Mutex;
use crate::allocators::simple::FixedSizeBlockAllocator;

// Static heap buffer embedded in the kernel binary
const HEAP_SIZE: usize = 256 * 1024 * 1024; // 2 MB
#[repr(align(4096))]
struct HeapBuffer([u8; HEAP_SIZE]);
static mut HEAP_BUFFER: HeapBuffer = HeapBuffer([0; HEAP_SIZE]);

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::new();

pub struct LockedHeap {
    inner: Mutex<Option<FixedSizeBlockAllocator>>,
}

impl LockedHeap {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) -> Result<(), &'static str> {
        let allocator = FixedSizeBlockAllocator::new();
        allocator.init(heap_start, heap_size)
            .map_err(|_| "Failed to initialize allocator")?;
        *self.inner.lock() = Some(allocator);
        Ok(())
    }
}

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.inner
            .lock()
            .as_ref()
            .map_or(core::ptr::null_mut(), |allocator| allocator.alloc(layout))
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(allocator) = self.inner.lock().as_ref() {
            allocator.dealloc(ptr, layout);
        }
    }
}

pub unsafe fn init_heap(_boot_info: &BootInfo) -> Result<(), &'static str> {
    use core::fmt::Write;
    let mut serial = crate::SERIAL.lock();
    
    let heap_start = HEAP_BUFFER.0.as_ptr() as usize;
    let heap_size = HEAP_SIZE;
    
    let _ = writeln!(serial, "Heap: {:#x} - {:#x} ({} KB)", 
                    heap_start, heap_start + heap_size, heap_size / 1024);
    
    ALLOCATOR.init(heap_start, heap_size)?;
    
    Ok(())
}

pub fn memory_stats(boot_info: &BootInfo) {
    use core::fmt::Write;
    let mut serial = crate::SERIAL.lock();
    
    let _ = writeln!(serial, "\n=== Memory Map ===");
    
    let regions = boot_info.memory_regions.as_slice();
    let mut total_usable = 0u64;
    let mut total_reserved = 0u64;
    
    for region in regions {
        let size = region.end - region.start;
        let kind_str = match region.kind {
            MemoryRegionKind::Usable => {
                total_usable += size;
                "Usable"
            }
            MemoryRegionKind::Bootloader => {
                total_reserved += size;
                "Bootloader"
            }
            MemoryRegionKind::UnknownBios(_) => {
                total_reserved += size;
                "BIOS"
            }
            MemoryRegionKind::UnknownUefi(_) => {
                total_reserved += size;
                "UEFI"
            }
            _ => {
                total_reserved += size;
                "Reserved"
            }
        };
        
        let _ = writeln!(
            serial,
            "  {:#018x} - {:#018x} ({:>8} KB) [{}]",
            region.start,
            region.end,
            size / 1024,
            kind_str
        );
    }
    
    let _ = writeln!(serial, "\nTotal Usable:   {} MB", total_usable / (1024 * 1024));
    let _ = writeln!(serial, "Total Reserved: {} MB", total_reserved / (1024 * 1024));
}