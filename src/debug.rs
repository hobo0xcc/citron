use crate::arch::target::debug;

pub enum Status {
    Pass,
    Fail,
}

pub fn exit_qemu(code: u32, status: Status) -> ! {
    #[cfg(target_arch = "riscv64")]
    {
        let exit_status = match status {
            Status::Pass => debug::QemuExitStatus::Pass,
            Status::Fail => debug::QemuExitStatus::Fail,
        };
        debug::exit_qemu(code, exit_status);
    }

    loop {}
}
