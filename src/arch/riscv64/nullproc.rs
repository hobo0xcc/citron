#[repr(align(4096))]
pub extern "C" fn null_proc() {
    loop {}
}
