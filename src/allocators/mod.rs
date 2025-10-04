
pub mod simple;
use core::alloc::Layout;

#[global_allocator]
static ALLOCATOR: simple::BumpAllocator = simple::BumpAllocator::new();

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    loop {
        unsafe { core::arch::x86_64::_mm_pause(); }
    }
}