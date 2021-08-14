use crate::*;

#[repr(align(4096))]
#[no_mangle]
pub extern "C" fn test_proc1() {
    let mut str_arr = ['A' as u8];
    loop {
        unsafe {
            asm!("mv a2, {}", in(reg)(&mut str_arr as *mut u8));
            asm!("li a0, 1", "li a1, 0", "li a3, 1", "ecall",);
            asm!("li a0, 35", "li a1, 10", "ecall");
            return;
        }
    }
    // loop {}
}

#[repr(align(4096))]
#[no_mangle]
pub extern "C" fn test_proc2() {
    let mut str_arr = ['B' as u8];
    loop {
        unsafe {
            asm!("mv a2, {}", in(reg)(&mut str_arr as *mut u8));
            asm!("li a0, 1", "li a1, 0", "li a3, 1", "ecall",);
            asm!("li a0, 35", "li a1, 10", "ecall");
            return;
        }
    }
    // loop {}
}

#[repr(align(4096))]
#[no_mangle]
pub extern "C" fn test_proc3() {
    let mut str_arr = ['C' as u8];
    loop {
        unsafe {
            asm!("mv a2, {}", in(reg)(&mut str_arr as *mut u8));
            asm!("li a0, 1", "li a1, 0", "li a3, 1", "ecall",);
            asm!("li a0, 35", "li a1, 100", "ecall");
        }
    }
    // loop {}
}

#[no_mangle]
pub extern "C" fn kmain() {
    let mut hart_id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg)hart_id);
    }
    if hart_id != 0 {
        loop {}
    }
    // Init kernel
    unsafe {
        init::init_all();
    }

    println!("Initialization done");
    println!("Hello, citron!");

    let pm = unsafe { process::process_manager() };
    {
        let pid1 = pm.create_process("test_proc1", 1);
        println!("pid1: {:#x}", pid1);
        pm.load_program(pid1, test_proc1 as usize, 0x1000);
        pm.ready(pid1);
    }
    {
        let pid2 = pm.create_process("test_proc2", 1);
        println!("pid2: {:#x}", pid2);
        pm.load_program(pid2, test_proc2 as usize, 0x1000);
        pm.ready(pid2);
    }
    {
        let pid3 = pm.create_process("test_proc3", 1);
        println!("pid3: {:#x}", pid3);
        pm.load_program(pid3, test_proc3 as usize, 0x1000);
        pm.ready(pid3);
    }

    arch::target::interrupt::timer_interrupt_on();

    pm.schedule();
    loop {}
}
