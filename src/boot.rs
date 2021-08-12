#![no_std]
#![feature(
    panic_info_message,
    llvm_asm,
    global_asm,
    asm,
    start,
    stmt_expr_attributes
)]
#![no_main]
extern crate citron;

#[no_mangle]
pub extern "C" fn _entry() {
    unsafe {
        #[cfg(all(target_arch = "riscv64"))]
        asm!("j _bootriscv64");
    }
}

// #[no_mangle]
// pub extern "C" fn _entry() {
//     unsafe {
//         asm!(
//             "    la sp, _stack_start",
//             "    li a0, 0x1000",
//             "    csrr a1, mhartid",
//             "    addi a1, a1, 1",
//             "    mul a0, a0, a1",
//             "    add sp, sp, a0",
//             "    call start",
//             "spin:",
//             "    j spin",
//         );
//     }
// }
