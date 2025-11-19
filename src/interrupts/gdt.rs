// gdt.rs
use spin::Lazy;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

static mut DOUBLE_FAULT_STACK: [u8; 4096] = [0; 4096];

static TSS: Lazy<TaskStateSegment> = Lazy::new(|| {
    let mut tss = TaskStateSegment::new();
    
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        let stack_start = unsafe { DOUBLE_FAULT_STACK.as_ptr() as u64 };
        let stack_end = stack_start + 4096;
        VirtAddr::new(stack_end)
    };
    
    tss
});

struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

static GDT: Lazy<(GlobalDescriptorTable, Selectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    
    let code_selector = gdt.append(Descriptor::kernel_code_segment());
    let data_selector = gdt.append(Descriptor::kernel_data_segment());
    let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
    
    (
        gdt,
        Selectors {
            code_selector,
            data_selector,
            tss_selector,
        },
    )
});

pub fn init() {
    let (ref gdt, ref selectors) = *GDT;
    gdt.load();
    
    unsafe {
        use x86_64::instructions::segmentation::{CS, DS, ES, SS, Segment};
        
        // Set code segment
        CS::set_reg(selectors.code_selector);
        
        // Set data segments - THIS WAS MISSING!
        DS::set_reg(selectors.data_selector);
        ES::set_reg(selectors.data_selector);
        SS::set_reg(selectors.data_selector);
        
        // Load TSS
        x86_64::instructions::tables::load_tss(selectors.tss_selector);
    }
}
