#![no_std]
#![feature(
    panic_info_message,
    llvm_asm,
    global_asm,
    asm,
    start,
    alloc_error_handler,
    fn_align,
    once_cell
)]
#![allow(named_asm_labels)]
// #![no_implicit_prelude]

// extern crate alloc;
// extern crate array_init;
// extern crate embedded_graphics;
// extern crate fontdue;
// extern crate goblin;
// extern crate hashbrown;
// extern crate intrusive_collections;
// extern crate libm;
// extern crate linked_list_allocator;
// extern crate tiny_skia;
// extern crate tinybmp;
// extern crate volatile_register;
// extern crate riscv;

// pub mod allocator;
pub mod arch;
// pub mod fs;
// pub mod graphics;
pub mod init;
pub mod kmain;
// pub mod process;

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
    }
}
