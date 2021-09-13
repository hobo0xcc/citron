#![no_std]
#![feature(
    panic_info_message,
    llvm_asm,
    global_asm,
    asm,
    start,
    alloc_error_handler,
    fn_align,
    once_cell,
    custom_test_frameworks,
    lang_items
)]
#![cfg_attr(test, no_main)]
#![allow(named_asm_labels)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
// #![no_implicit_prelude]

extern crate alloc;
extern crate array_init;
extern crate embedded_graphics;
extern crate fontdue;
extern crate goblin;
extern crate hashbrown;
extern crate intrusive_collections;
extern crate libm;
extern crate linked_list_allocator;
extern crate spin;
extern crate tiny_skia;
extern crate tinybmp;
extern crate volatile_register;

pub mod allocator;
pub mod arch;
pub mod debug;
pub mod fs;
pub mod graphics;
pub mod init;
pub mod kmain;
pub mod process;
pub mod spinlock;

#[macro_export]
macro_rules! test_harness {
    () => {
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

            println!("FAIL");

            debug::exit_qemu(1, debug::Status::Fail);
        }

        #[no_mangle]
        #[start]
        #[cfg(test)]
        #[link_section = ".text.boot"]
        pub extern "C" fn _entry() {
            unsafe {
                asm!("la a0, kmain_test");
                asm!("j _bootriscv64");
            }
        }

        #[no_mangle]
        pub extern "C" fn kmain_test() -> ! {
            unsafe {
                init::init_all();
            }
            #[cfg(test)]
            test_main();
            loop {}
        }
    };
}

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        println!("{}...\t", core::any::type_name::<T>());
        self();
        println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    debug::exit_qemu(0, debug::Status::Pass);
}

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
