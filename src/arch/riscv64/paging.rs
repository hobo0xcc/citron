use super::csr::Csr;
use super::layout::*;
use super::trampoline;
use alloc::alloc::{alloc_zeroed, dealloc};
use core::alloc::Layout;

use crate::*;

const PAGE_SIZE: usize = 4096;

// For Sv32
#[cfg(target_pointer_width = "32")]
const LEVELS: usize = 2;

// For Sv39
#[cfg(target_pointer_width = "64")]
const LEVELS: usize = 3;

#[cfg(target_pointer_width = "32")]
#[allow(non_camel_case_types)]
pub enum EntryBits {
    V,
    R,
    W,
    X,
    U,
    G,
    A,
    D,
    RSW,
    PPN_0,
    PPN_1,
}

#[cfg(target_pointer_width = "64")]
#[allow(non_camel_case_types)]
pub enum EntryBits {
    V,
    R,
    W,
    X,
    U,
    G,
    A,
    D,
    RSW,
    PPN_0,
    PPN_1,
    PPN_2,
}

#[cfg(target_pointer_width = "32")]
impl EntryBits {
    pub fn val(&self) -> usize {
        match *self {
            EntryBits::V => 1 << 0,
            EntryBits::R => 1 << 1,
            EntryBits::W => 1 << 2,
            EntryBits::X => 1 << 3,
            EntryBits::U => 1 << 4,
            EntryBits::G => 1 << 5,
            EntryBits::A => 1 << 6,
            EntryBits::D => 1 << 7,
            EntryBits::RSW => 0x3 << 8,
            EntryBits::PPN_0 => 0x3ff << 10,
            EntryBits::PPN_1 => 0xfff << 20,
        }
    }

    pub fn get_field(&self, entry: usize) -> usize {
        match *self {
            EntryBits::V => entry & self.val() >> 0,
            EntryBits::R => entry & self.val() >> 1,
            EntryBits::W => entry & self.val() >> 2,
            EntryBits::X => entry & self.val() >> 3,
            EntryBits::U => entry & self.val() >> 4,
            EntryBits::G => entry & self.val() >> 5,
            EntryBits::A => entry & self.val() >> 6,
            EntryBits::D => entry & self.val() >> 7,
            EntryBits::RSW => entry & self.val() >> 8,
            EntryBits::PPN_0 => entry & self.val() >> 10,
            EntryBits::PPN_1 => entry & self.val() >> 20,
        }
    }
}

#[cfg(target_pointer_width = "64")]
impl EntryBits {
    pub fn val(&self) -> usize {
        match *self {
            EntryBits::V => 1 << 0,
            EntryBits::R => 1 << 1,
            EntryBits::W => 1 << 2,
            EntryBits::X => 1 << 3,
            EntryBits::U => 1 << 4,
            EntryBits::G => 1 << 5,
            EntryBits::A => 1 << 6,
            EntryBits::D => 1 << 7,
            EntryBits::RSW => 0x3 << 8,
            EntryBits::PPN_0 => 0x1ff << 10,
            EntryBits::PPN_1 => 0x1ff << 19,
            EntryBits::PPN_2 => 0x3ffffff << 28,
        }
    }

    pub fn get_field(&self, entry: usize) -> usize {
        match *self {
            EntryBits::V => entry & self.val() >> 0,
            EntryBits::R => entry & self.val() >> 1,
            EntryBits::W => entry & self.val() >> 2,
            EntryBits::X => entry & self.val() >> 3,
            EntryBits::U => entry & self.val() >> 4,
            EntryBits::G => entry & self.val() >> 5,
            EntryBits::A => entry & self.val() >> 6,
            EntryBits::D => entry & self.val() >> 7,
            EntryBits::RSW => entry & self.val() >> 8,
            EntryBits::PPN_0 => entry & self.val() >> 10,
            EntryBits::PPN_1 => entry & self.val() >> 19,
            EntryBits::PPN_2 => entry & self.val() >> 28,
        }
    }
}

#[repr(C)]
pub struct Entry {
    entry: usize,
}

impl Entry {
    pub fn get_entry(&self) -> usize {
        self.entry
    }

    pub fn set_entry(&mut self, entry: usize) {
        self.entry = entry;
    }

    pub fn bits(&self, eb: EntryBits) -> usize {
        eb.get_field(self.get_entry())
    }

    pub fn is_valid(&self) -> bool {
        self.bits(EntryBits::V) != 0
    }

    pub fn is_invalid(&self) -> bool {
        !self.is_valid()
    }

    pub fn is_leaf(&self) -> bool {
        self.get_entry() & 0xe != 0
    }

    pub fn is_branch(&self) -> bool {
        !self.is_leaf()
    }
}

#[cfg(target_pointer_width = "32")]
pub struct Table {
    // Sv32 page tables consist of 2^10 = 1024 page-table entries
    pub entries: [Entry; 1024],
}

#[cfg(target_pointer_width = "64")]
pub struct Table {
    // Sv39 page tables contain 2^9 page table entries
    pub entries: [Entry; 512],
}

#[cfg(target_pointer_width = "32")]
impl Table {
    pub fn len() -> usize {
        1024
    }
}

#[cfg(target_pointer_width = "64")]
impl Table {
    pub fn len() -> usize {
        512
    }
}

pub fn map(root: &mut Table, vaddr: usize, paddr: usize, bits: usize, level: usize) {
    assert!(bits & 0xe != 0);

    #[cfg(target_pointer_width = "32")]
    let vpn = [(vaddr >> 12) & 0x3ff, (vaddr >> 22) & 0x3ff];
    #[cfg(target_pointer_width = "64")]
    let vpn = [
        (vaddr >> 12) & 0x1ff,
        (vaddr >> 21) & 0x1ff,
        (vaddr >> 30) & 0x1ff,
    ];

    #[cfg(target_pointer_width = "32")]
    let ppn = [(paddr >> 12) & 0x3ff, (paddr >> 22) & 0xfff];
    #[cfg(target_pointer_width = "64")]
    let ppn = [
        (paddr >> 12) & 0x1ff,
        (paddr >> 21) & 0x1ff,
        (paddr >> 30) & 0x3ffffff,
    ];

    let mut v = &mut root.entries[vpn[LEVELS - 1]];

    for i in (level..(LEVELS - 1)).rev() {
        if v.is_invalid() {
            let layout = Layout::from_size_align(0x1000, 0x1000).unwrap();
            let page = unsafe { alloc_zeroed(layout) };
            v.set_entry((page as usize >> 2) | EntryBits::V.val());
        }
        let entry = ((v.get_entry() & !0x3ff) << 2) as *mut Entry;
        v = unsafe { entry.add(vpn[i]).as_mut().unwrap() };
    }

    #[cfg(target_pointer_width = "32")]
    let entry = (ppn[1] << 20) | (ppn[0] << 10) | bits | EntryBits::V.val();
    #[cfg(target_pointer_width = "64")]
    let entry = (ppn[2] << 28) | (ppn[1] << 19) | (ppn[0] << 10) | bits | EntryBits::V.val();

    v.set_entry(entry);
}

pub fn unmap(root: &mut Table) {
    let page_layout = Layout::from_size_align(0x1000, 0x1000).unwrap();
    #[cfg(target_pointer_width = "32")]
    for lv2 in 0..Table::len() {
        let ref entry_lv2 = root.entries[lv2];
        if entry_lv2.is_valid() && entry_lv2.is_branch() {
            let memaddr_lv1 = (entry_lv2.get_entry() & !0x3ff) << 2;
            let table_lv1 = unsafe { (memaddr_lv1 as *mut Table).as_mut().unwrap() };
            for lv1 in 0..Table::len() {
                let ref entry_lv1 = table_lv1.entries[lv1];
                if entry_lv1.is_valid() && entry_lv1.is_branch() {
                    let memaddr_lv0 = (entry_lv1.get_entry() & !0x3ff) << 2;
                    unsafe {
                        dealloc(memaddr_lv0 as *mut u8, page_layout);
                    }
                }
            }
            unsafe {
                dealloc(memaddr_lv1 as *mut u8, page_layout);
            }
        }
    }

    #[cfg(target_pointer_width = "64")]
    for lv3 in 0..Table::len() {
        let ref entry_lv3 = root.entries[lv3];
        if entry_lv3.is_valid() && entry_lv3.is_branch() {
            let memaddr_lv2 = (entry_lv3.get_entry() & !0x3ff) << 2;
            let table_lv2 = unsafe { (memaddr_lv2 as *mut Table).as_mut().unwrap() };
            for lv2 in 0..Table::len() {
                let ref entry_lv2 = table_lv2.entries[lv2];
                if entry_lv2.is_valid() && entry_lv2.is_branch() {
                    let memaddr_lv1 = (entry_lv2.get_entry() & !0x3ff) << 2;
                    let table_lv1 = unsafe { (memaddr_lv1 as *mut Table).as_mut().unwrap() };
                    for lv1 in 0..Table::len() {
                        let ref entry_lv1 = table_lv1.entries[lv1];
                        if entry_lv1.is_valid() && entry_lv1.is_branch() {
                            let memaddr_lv0 = (entry_lv1.get_entry() & !0x3ff) << 2;
                            unsafe {
                                dealloc(memaddr_lv0 as *mut u8, page_layout);
                            }
                        }
                    }
                    unsafe {
                        dealloc(memaddr_lv1 as *mut u8, page_layout);
                    }
                }
            }
            unsafe {
                dealloc(memaddr_lv2 as *mut u8, page_layout);
            }
        }
    }
}

pub fn virt_to_phys(root: &Table, vaddr: usize) -> Option<usize> {
    #[cfg(target_pointer_width = "32")]
    let vpn = [(vaddr >> 12) & 0x3ff, (vaddr >> 22) & 0x3ff];
    #[cfg(target_pointer_width = "64")]
    let vpn = [
        (vaddr >> 12) & 0x1ff,
        (vaddr >> 21) & 0x1ff,
        (vaddr >> 30) & 0x1ff,
    ];

    #[cfg(target_pointer_width = "32")]
    let mut v = &root.entries[vpn[1]];
    #[cfg(target_pointer_width = "64")]
    let mut v = &root.entries[vpn[2]];

    for i in (0..=(LEVELS - 1)).rev() {
        if v.is_invalid() {
            break;
        } else if v.is_leaf() {
            #[cfg(target_pointer_width = "32")]
            let off_mask: usize = (1 << (12 + i * 10)) - 1;
            #[cfg(target_pointer_width = "64")]
            let off_mask: usize = (1 << (12 + i * 9)) - 1;
            let vaddr_pgoff = vaddr & off_mask;
            let addr = ((v.get_entry() << 2) as usize) & !off_mask;
            return Some(addr | vaddr_pgoff);
        }

        let entry = ((v.get_entry() & !0x3ff) << 2) as *const Entry;
        v = unsafe { entry.add(vpn[i - 1]).as_ref().unwrap() };
    }

    None
}

fn align_val(val: usize, align: usize) -> usize {
    let t = (1usize << align) - 1;
    (val + t) & !t
}

pub fn map_range(root: &mut Table, vaddr: usize, paddr: usize, size: usize, bits: usize) {
    let mut memaddr = paddr & !(PAGE_SIZE - 1);
    let mut memaddr_v = vaddr & !(PAGE_SIZE - 1);
    let num_kb_pages = (align_val(memaddr + size, 12) - memaddr) / PAGE_SIZE;
    for _ in 0..num_kb_pages {
        map(root, memaddr_v, memaddr, bits, 0);
        memaddr += PAGE_SIZE;
        memaddr_v += PAGE_SIZE;
    }
}

pub fn id_map_range(root: &mut Table, start: usize, end: usize, bits: usize) {
    let mut memaddr = start & !(PAGE_SIZE - 1);
    let num_kb_pages = (align_val(end, 12) - memaddr) / PAGE_SIZE;
    for _ in 0..num_kb_pages {
        map(root, memaddr, memaddr, bits, 0);
        memaddr += PAGE_SIZE;
    }
}

pub extern "C" fn init() {
    println!();
    println!("SECTION _text_start\t: {:#010x}", _text_start as usize);
    println!("SECTION _text_end\t: {:#010x}", _text_end as usize);
    println!("SECTION _rodata_start\t: {:#010x}", _rodata_start as usize);
    println!("SECTION _rodata_end\t: {:#010x}", _rodata_end as usize);
    println!("SECTION _data_start\t: {:#010x}", _data_start as usize);
    println!("SECTION _data_end\t: {:#010x}", _data_end as usize);
    println!("SECTION _bss_start\t: {:#010x}", _bss_start as usize);
    println!("SECTION _bss_end\t: {:#010x}", _bss_end as usize);
    println!("SECTION _stack_start\t: {:#010x}", _stack_start as usize);
    println!("SECTION _stack_end\t: {:#010x}", _stack_end as usize);
    println!("SECTION _heap_start\t: {:#010x}", _heap_start as usize);
    println!("SECTION _heap_end\t: {:#010x}", _heap_end as usize);
    println!("SECTION _clint_start\t: {:#010x}", _clint_start as usize);
    println!("SECTION _clint_end\t: {:#010x}", _clint_end as usize);
    println!("SECTION _plic_start\t: {:#010x}", _plic_start as usize);
    println!("SECTION _plic_end\t: {:#010x}", _plic_end as usize);
    println!("SECTION _uart0_start\t: {:#010x}", _uart0_start as usize);
    println!("SECTION _uart0_end\t: {:#010x}", _uart0_end as usize);
    println!("SECTION _virtio_start\t: {:#010x}", _virtio_start as usize);
    println!("SECTION _virtio_end\t: {:#010x}", _virtio_end as usize);
    println!("SECTION _fw_cfg_start\t: {:#010x}", _fw_cfg_start as usize);
    println!("SECTION _fw_cfg_end\t: {:#010x}", _fw_cfg_end as usize);
    println!();

    let root_ptr = unsafe { alloc_zeroed(Layout::from_size_align(0x1000, 0x1000).unwrap()) };
    let root = unsafe { (root_ptr.cast::<Table>()).as_mut().unwrap() };

    id_map_range(
        root,
        _text_start as usize,
        _text_end as usize,
        EntryBits::R.val() | EntryBits::X.val(),
    );

    id_map_range(
        root,
        _rodata_start as usize,
        _rodata_end as usize,
        EntryBits::R.val(),
    );

    id_map_range(
        root,
        _data_start as usize,
        _data_end as usize,
        EntryBits::R.val() | EntryBits::W.val(),
    );

    id_map_range(
        root,
        _bss_start as usize,
        _bss_end as usize,
        EntryBits::R.val() | EntryBits::W.val(),
    );

    id_map_range(
        root,
        _stack_start as usize,
        _stack_end as usize,
        EntryBits::R.val() | EntryBits::W.val(),
    );

    id_map_range(
        root,
        _heap_start as usize,
        _heap_end as usize,
        EntryBits::R.val() | EntryBits::W.val(),
    );

    // CLINT
    id_map_range(
        root,
        _clint_start as usize,
        _clint_end as usize,
        EntryBits::R.val() | EntryBits::W.val(),
    );

    // PLIC
    id_map_range(
        root,
        _plic_start as usize,
        _plic_end as usize,
        EntryBits::R.val() | EntryBits::W.val(),
    );

    // UART
    id_map_range(
        root,
        _uart0_start as usize,
        _uart0_end as usize,
        EntryBits::R.val() | EntryBits::W.val(),
    );

    // VIRTIO
    id_map_range(
        root,
        _virtio_start as usize,
        _virtio_end as usize,
        EntryBits::R.val() | EntryBits::W.val(),
    );

    // FW_CFG
    id_map_range(
        root,
        _fw_cfg_start as usize,
        _fw_cfg_end as usize,
        EntryBits::R.val() | EntryBits::W.val(),
    );

    map(
        root,
        trampoline::TRAMPOLINE,
        trampoline::trampoline as usize,
        EntryBits::R.val() | EntryBits::X.val(),
        0,
    );

    // Enable paging
    let root_ppn = (root_ptr as usize) >> 12;
    #[cfg(target_pointer_width = "32")]
    let satp_val = 1_usize << 31 | root_ppn;
    #[cfg(target_pointer_width = "64")]
    let satp_val = 8_usize << 60 | root_ppn;

    unsafe {
        Csr::Satp.write(satp_val);
        asm!("sfence.vma zero, zero");
    }
}
