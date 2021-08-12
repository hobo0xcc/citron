use super::csr::Csr;
use crate::arch::target::*;
use crate::*;

unsafe extern "C" fn pmp_init() {
    let pmpaddr0 = (!0_usize) >> 10;
    asm!("csrw pmpaddr0, {}", in(reg)pmpaddr0);
    let mut pmpcfg0 = 0_usize;
    pmpcfg0 |= 3 << 3;
    pmpcfg0 |= 1 << 2;
    pmpcfg0 |= 1 << 1;
    pmpcfg0 |= 1 << 0;
    asm!("csrw pmpcfg0, {}", in(reg)pmpcfg0);
}

#[no_mangle]
pub unsafe extern "C" fn start() {
    // uart::init();

    // We want to enter supervisor mode to execute kernel code.
    // So we use MRET instruction at the end of this function to accomplish this purpose.

    // Initialization of RV64 and RV32 Machine.

    // from: RISC-V ISM Volume 2, 3.1.6.1
    //
    // xPIE holds the value of the interrupt-enable bit active prior to the trap,
    // and xPP holds the previous privilege mode.
    // The xPP fields can only hold privilege modes up to x, so MPP is two bits wide,
    // SPP is one bit wide, and UPP is implicitly zero.
    // When a trap is taken from privilege mode y into privilege mode x,
    // xPIE is set to the value of xIE; xIE is set to 0; and xPP is set to y.
    //
    // The MRET, SRET, or URET instructions are used to return from traps in
    // M-mode, S-mode, or U-mode respectively. When executing an xRET instruction,
    // supposing xPP holds the value y, xIE is set to xPIE; the privilege mode is changed to y;
    // xPIE is set to 1; and xPP is set to U (or M if user-mode is not supported).

    // When executing an MRET instruction:
    // y = MPP;
    // MIE = MPIE;
    // Privilege mode = y;
    // MPIE = 1;
    // MPP = U;

    // Set next mode to supervisor.
    let mut mstatus_val = Csr::Mstatus.read();
    mstatus_val &= !(0x3_usize << 11);
    mstatus_val |= 0x1_usize << 11; // MPP

    // Enable interrupt for supervisor.
    mstatus_val |= 1 << 5; // SPIE
    mstatus_val |= 1 << 7; // MPIE
    mstatus_val |= 1 << 1; // SIE
    mstatus_val |= 1 << 3; // MIE
    Csr::Mstatus.write(mstatus_val);

    let mut sstatus_val = Csr::Sstatus.read();
    sstatus_val |= 1 << 5; // SPIE
    sstatus_val |= 1 << 1; // SIE
    Csr::Sstatus.write(sstatus_val);

    // Jump to main
    let mepc_val = kmain::kmain as usize;
    Csr::Mepc.write(mepc_val);

    pmp_init();

    // Disable paging.
    asm!("csrw satp, zero");

    // Delegate all interrupts and exceptions.
    asm!("li t0, 0xffff");
    asm!("csrw mideleg, t0");
    asm!("li t0, 0xffff");
    asm!("csrw medeleg, t0");

    clint::init();

    let mut mie_val = Csr::Mie.read();
    mie_val |= 1 << 7;
    Csr::Mie.write(mie_val);

    asm!("csrr tp, mhartid");

    Csr::Stvec.write(trap::kernelvec as usize);

    asm!("mret");

    loop {}
}
