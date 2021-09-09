#[no_mangle]
pub extern "C" fn _bootriscv64() {
    unsafe {
        asm!(include_str!("boot.S"));
    }
}
