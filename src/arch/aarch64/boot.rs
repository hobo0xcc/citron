#[no_mangle]
pub extern "C" fn _bootaarch64() {
    unsafe {
        asm!(
            include_str!("boot.S")
        );
    }
}
