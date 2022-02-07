#![feature(panic_info_message)]
#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![reexport_test_harness_main = "test_main"]
#![test_runner(citron::test_runner)]

use citron::*;
use core::arch::asm;

test_harness!();

fn fib(n: i32) -> i32 {
    if n < 2 {
        1
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

fn goodbye() {
    let pm = unsafe { process::process_manager() };
    for _ in 0..10 {
        println!("Goodbye");
        pm.sleep(pm.running, 10).unwrap();
    }

    pm.kill(pm.running).unwrap();
}

fn hello() {
    let pm = unsafe { process::process_manager() };
    for _ in 0..10 {
        println!("Hello");
        pm.sleep(pm.running, 10).unwrap();
    }

    pm.kill(pm.running).unwrap();
}

#[test_case]
fn test_hello_goodbye() {
    arch::target::interrupt::interrupt_on();
    arch::target::interrupt::timer_interrupt_on();

    let pm = unsafe { process::process_manager() };
    pm.defer_schedule(process::DeferCommand::Start).unwrap();
    let pid1 = pm
        .create_kernel_process("hello", 1, hello as usize)
        .unwrap();
    pm.ready(pid1).unwrap();
    let pid2 = pm
        .create_kernel_process("goodbye", 1, goodbye as usize)
        .unwrap();
    pm.ready(pid2).unwrap();

    pm.defer_schedule(process::DeferCommand::Stop).unwrap();

    while pm.get_process_state(pid1).unwrap() == process::State::Sleep
        || pm.get_process_state(pid2).unwrap() == process::State::Sleep
    {
        pm.schedule().unwrap();
    }
}
