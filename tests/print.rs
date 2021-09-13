#![feature(panic_info_message)]
#![no_std]
#![no_main]
#![feature(asm)]
#![feature(custom_test_frameworks)]
#![reexport_test_harness_main = "test_main"]
#![test_runner(citron::test_runner)]

use citron::process::process_manager;
use citron::*;

test_harness!();

#[test_case]
fn test_print() {
    println!("Hello, world!");
    assert_eq!(2, 2);
}
