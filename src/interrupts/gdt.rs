//! Global Descriptor Table and Task State Segment
// The GDT (Global Descriptor Table) is a legacy x86 structure that defines
// memory segments. In 64-bit mode, segmentation is mostly disabled, but we
// still need the GDT for:
//   1. Code segment selector (required by the CPU)
//   2. TSS (Task State Segment) entry
//
// What is the TSS?
// ----------------
// The TSS (Task State Segment) tells the CPU where to find special stacks.
// Most importantly, it provides separate stacks for handling critical errors
// like double faults. Without this, a double fault would try to use the same
// corrupted stack that caused it, leading to a triple fault (reboot).

use spin::Lazy;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

// Index into the TSS's interrupt stack table
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

// A dedicated stack for double fault handling
static mut DOUBLE_FAULT_STACK: [u8; 4096] = [0; 4096];

static TSS: Lazy<TaskStateSegment> = Lazy::new(|| {
    let mut tss = TaskStateSegment::new();
    
    // Set up the interrupt stack table entry for double faults
    // The stack grows downward, so we point to the END of the array
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        let stack_start = unsafe { DOUBLE_FAULT_STACK.as_ptr() as u64 };
        let stack_end = stack_start + 4096;
        VirtAddr::new(stack_end)
    };
    
    tss
});

// Struct to hold both the GDT and the selectors we need
struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

// Global Descriptor Table - holds segment descriptors
static GDT: Lazy<(GlobalDescriptorTable, Selectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    
    // Add a code segment (required for 64-bit mode)
    let code_selector = gdt.append(Descriptor::kernel_code_segment());
    
    // Add the TSS segment
    let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
    
    // Return both the GDT and the selectors we'll need later
    (
        gdt,
        Selectors {
            code_selector,
            tss_selector,
        },
    )
});

pub fn init() {
    let (ref gdt, ref selectors) = *GDT;
    gdt.load();
    
    unsafe {
        use x86_64::instructions::segmentation::{CS, Segment};
        CS::set_reg(selectors.code_selector);
        x86_64::instructions::tables::load_tss(selectors.tss_selector);
    }
}