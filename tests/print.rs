#![feature(panic_info_message)]
#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![reexport_test_harness_main = "test_main"]
#![test_runner(citron::test_runner)]

use citron::*;
use core::arch::asm;

test_harness!();

#[test_case]
fn test_print() {
    println!("Hello, world!");
    assert_eq!(2, 2);
}

fn fib(n: i32) -> i32 {
    if n < 2 {
        1
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

#[test_case]
fn test_function() {
    assert_eq!(fib(10), 89);
}
