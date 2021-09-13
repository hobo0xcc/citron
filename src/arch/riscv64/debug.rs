use super::layout::QEMU_VIRT_TEST;

#[repr(u32)]
pub enum QemuExitStatus {
    Pass = 0x5555,
    Fail = 0x3333,
}

pub fn exit_qemu(code: u32, status: QemuExitStatus) {
    unsafe {
        let virt_test = QEMU_VIRT_TEST as *mut u32;
        let val = status as u32 | code << 16;
        virt_test.write_volatile(val);
    }
}
