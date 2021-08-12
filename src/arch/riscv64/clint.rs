use super::csr::Csr;
use super::*;

pub const INTERVAL: usize = 100000;
pub const MSIP: usize = 0x0;
pub const MTIME: usize = 0xbff8;
pub const MTIMECMP: usize = 0x4000;
static mut TIMER_SCRATCH: [usize; 5] = [0; 5];

#[repr(align(4))]
pub unsafe extern "C" fn timervec() {
    #[cfg(target_pointer_width = "32")]
    asm!(
        ".align 4"
        "csrrw a0, mscratch, a0",
        "sw a1, 0(a0)",
        "sw a2, 8(a0)",
        "sw a3, 16(a0)",
        "lw a1, 24(a0)",
        "lw a2, 32(a0)",
        "lw a3, 0(a1)",
        "add a3, a3, a2",
        "sw a3, 0(a1)",
        "li a1, 2",
        "csrw sip, a1",
        "lw a3, 16(a0)",
        "lw a2, 8(a0)",
        "lw a1, 0(a0)",
        "csrrw a0, mscratch, a0",
        "mret",
    );

    #[cfg(target_pointer_width = "64")]
    asm!(
        "csrrw a0, mscratch, a0",
        "sd a1, 0(a0)",
        "sd a2, 8(a0)",
        "sd a3, 16(a0)",
        "ld a1, 24(a0)",
        "ld a2, 32(a0)",
        "ld a3, 0(a1)",
        "add a3, a3, a2",
        "sd a3, 0(a1)",
        "li a1, 2",
        "csrw sip, a1",
        "ld a3, 16(a0)",
        "ld a2, 8(a0)",
        "ld a1, 0(a0)",
        "csrrw a0, mscratch, a0",
        "mret",
    );
}

pub unsafe extern "C" fn init() {
    let mtimecmp = (layout::_clint_start as usize + MTIMECMP) as *mut usize;
    let mtime = (layout::_clint_start as usize + MTIME) as *mut usize;
    *mtimecmp = *mtime + INTERVAL;
    let scratch = TIMER_SCRATCH.as_mut_ptr();
    // Save context
    *(scratch.add(3)) = layout::_clint_start as usize + MTIMECMP;
    *(scratch.add(4)) = INTERVAL;

    Csr::Mscratch.write(scratch as usize);
    Csr::Mtvec.write(timervec as usize);
}
