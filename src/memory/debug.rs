use crate::{memory::access_page_table, println};
use x86_64::{registers::control::Cr3, structures::paging::PageTableFlags, VirtAddr};

pub fn debug_page_walk(virt: VirtAddr) {
    let va_u64 = virt.as_u64();
    let p4_index = ((va_u64 >> 39) & 0x1FF) as usize;
    let p3_index = ((va_u64 >> 30) & 0x1FF) as usize;
    let p2_index = ((va_u64 >> 21) & 0x1FF) as usize;
    let p1_index = ((va_u64 >> 12) & 0x1FF) as usize;

    let (cr3_frame, _) = Cr3::read();
    let cr3_phys = cr3_frame.start_address();

    println!("Page walk for virt {:#x}:", va_u64);
    println!("  CR3 P4 frame phys: {:#x}", cr3_phys.as_u64());

    // Walk P4
    let p4_table = unsafe { access_page_table(cr3_phys) };
    let p4_entry = &p4_table[p4_index];
    let p4_addr = p4_entry.addr().as_u64();
    let p4_flags = p4_entry.flags();
    // Check bit 63 (NX bit) directly
    let p4_raw = unsafe { core::ptr::read_volatile(&p4_table[p4_index] as *const _ as *const u64) };
    let p4_nx = (p4_raw >> 63) & 1;
    println!(
        "  P4[{}] raw={:#x} NX={} flags={:?} addr={:#x}",
        p4_index, p4_raw, p4_nx, p4_flags, p4_addr
    );

    if !p4_flags.contains(PageTableFlags::PRESENT) {
        println!("  -> P4 entry not present, stopping walk");
        return;
    }

    let p3_phys = match p4_entry.frame() {
        Ok(f) => f.start_address(),
        Err(_) => {
            println!("  -> P4 entry has no valid frame");
            return;
        }
    };

    // Walk P3
    let p3_table = unsafe { access_page_table(p3_phys) };
    let p3_entry = &p3_table[p3_index];
    let p3_raw = unsafe { core::ptr::read_volatile(&p3_table[p3_index] as *const _ as *const u64) };
    let p3_nx = (p3_raw >> 63) & 1;
    println!(
        "  P3[{}] raw={:#x} NX={} flags={:?}",
        p3_index,
        p3_raw,
        p3_nx,
        p3_entry.flags()
    );

    if !p3_entry.flags().contains(PageTableFlags::PRESENT) {
        println!("  -> P3 entry not present, stopping walk");
        return;
    }

    let p2_phys = match p3_entry.frame() {
        Ok(f) => f.start_address(),
        Err(_) => {
            println!("  -> P3 entry has no valid frame");
            return;
        }
    };

    // Walk P2
    let p2_table = unsafe { access_page_table(p2_phys) };
    let p2_entry = &p2_table[p2_index];
    let p2_raw = unsafe { core::ptr::read_volatile(&p2_table[p2_index] as *const _ as *const u64) };
    let p2_nx = (p2_raw >> 63) & 1;
    println!(
        "  P2[{}] raw={:#x} NX={} flags={:?}",
        p2_index,
        p2_raw,
        p2_nx,
        p2_entry.flags()
    );

    if !p2_entry.flags().contains(PageTableFlags::PRESENT) {
        println!("  -> P2 entry not present, stopping walk");
        return;
    }

    let p1_phys = match p2_entry.frame() {
        Ok(f) => f.start_address(),
        Err(_) => {
            println!("  -> P2 entry has no valid frame");
            return;
        }
    };

    // Walk P1
    let p1_table = unsafe { access_page_table(p1_phys) };
    let p1_entry = &p1_table[p1_index];
    let p1_raw = unsafe { core::ptr::read_volatile(&p1_table[p1_index] as *const _ as *const u64) };
    let p1_nx = (p1_raw >> 63) & 1;
    println!(
        "  P1[{}] raw={:#x} NX={} flags={:?}",
        p1_index,
        p1_raw,
        p1_nx,
        p1_entry.flags()
    );

    if p1_entry.flags().contains(PageTableFlags::PRESENT) {
        if p1_nx == 1 {
            println!("  -> Page is PRESENT but NX bit is SET (not executable)!");
        } else {
            println!("  -> Page is PRESENT and executable (NX=0)");
        }
    } else {
        println!("  -> P1 entry not present");
    }
}
