#![feature(
    panic_info_message,
    llvm_asm,
    global_asm,
    asm,
    start,
    stmt_expr_attributes,
    lang_items,
    custom_test_frameworks
)]
#![test_runner(citron::test_runner)]
#![no_std]
#![no_main]
#![reexport_test_harness_main = "test_main"]
extern crate citron;

use citron::*;

#[no_mangle]
#[start]
#[cfg(not(test))]
pub extern "C" fn _entry() -> ! {
    #[cfg(all(target_arch = "riscv64"))]
    unsafe {
        asm!("la a0, kmain");
        asm!("j _bootriscv64");
    }
    #[cfg(all(target_arch = "aarch64"))]
    unsafe {
        asm!("b _bootaarch64");
    }

    loop {}
}

#[no_mangle]
#[start]
#[cfg(test)]
#[link_section = ".text.boot"]
pub extern "C" fn _entry() -> ! {
    #[cfg(all(target_arch = "riscv64"))]
    unsafe {
        asm!("la a0, kmain_test");
        asm!("j _bootriscv64");
    }
    #[cfg(all(target_arch = "aarch64"))]
    unsafe {
        asm!("b _bootaarch64");
    }

    loop {}
}

#[no_mangle]
pub extern "C" fn kmain_test() {
    #[cfg(test)]
    test_main();
    debug::exit_qemu(0, debug::Status::Pass);
}

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

    debug::exit_qemu(1, debug::Status::Fail);
}
