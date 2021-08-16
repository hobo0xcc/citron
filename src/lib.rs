#![no_std]
#![feature(
    panic_info_message,
    llvm_asm,
    global_asm,
    asm,
    start,
    alloc_error_handler,
    const_raw_ptr_to_usize_cast,
    fn_align,
    once_cell
)]
// #![no_implicit_prelude]

extern crate alloc;
extern crate array_init;
extern crate fatfs;
// extern crate fscommon;
extern crate intrusive_collections;
// extern crate riscv;

pub mod allocator;
pub mod arch;
pub mod init;
pub mod kmain;
pub mod process;

#[macro_export]
macro_rules! print {
    ($($args:tt)+) => {{
        use core::fmt::Write;
        let _ = write!(crate::arch::target::serial::get_serial(), $($args)+);
    }};
}
#[macro_export]
macro_rules! println
{
	() => ({
		print!("\r\n")
	});
	($fmt:expr) => ({
		print!(concat!($fmt, "\r\n"))
	});
	($fmt:expr, $($args:tt)+) => ({
		print!(concat!($fmt, "\r\n"), $($args)+)
	});
}

#[no_mangle]
extern "C" fn eh_personality() {}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    print!("Aborting: ");
    if let Some(p) = info.location() {
        println!(
            "line {}, file {}: {}",
            p.line(),
            p.file(),
            info.message().unwrap()
        );
    } else {
        println!("no information available.");
    }
    abort();
}

#[no_mangle]
extern "C" fn abort() -> ! {
    loop {
        unsafe {
            llvm_asm!("wfi"::::"volatile");
        }
    }
}
