#[no_mangle]
pub extern "C" fn _bootriscv64() {
    unsafe {
        asm!(
            "    la sp, _stack_start",
            "    li a0, 0x20000",
            "    csrr a1, mhartid",
            "    addi a1, a1, 1",
            "    mul a0, a0, a1",
            "    add sp, sp, a0",
            "    call start",
            "spin:",
            "    j spin",
        );
    }
}
