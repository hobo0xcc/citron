#[cfg(target_arch = "riscv64")]
pub mod riscv64;

#[cfg(target_arch = "riscv64")]
pub mod target {
    pub use super::riscv64::boot;
    pub use super::riscv64::clint;
    pub use super::riscv64::csr;
    pub use super::riscv64::fw_cfg;
    pub use super::riscv64::init;
    pub use super::riscv64::interrupt;
    pub use super::riscv64::layout;
    pub use super::riscv64::nullproc;
    pub use super::riscv64::paging;
    pub use super::riscv64::process;
    pub use super::riscv64::serial;
    pub use super::riscv64::start;
    pub use super::riscv64::syscall;
    pub use super::riscv64::trap;
}

pub mod paging;
pub mod serial;
pub mod syscall;
