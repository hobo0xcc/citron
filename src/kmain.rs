use crate::arch::riscv64::interrupt;
use crate::*;
use alloc::alloc::alloc_zeroed;
use core::alloc::Layout;

// kernel process
#[no_mangle]
pub extern "C" fn kproc() {
    let blk = unsafe { arch::target::virtio::block_device() };
    let layout = Layout::from_size_align(512, 512).unwrap();
    let buffer = unsafe { alloc_zeroed(layout) };
    blk.read_sector(0, buffer);
    println!("kernel.elf:");
    unsafe {
        for i in 0..512 {
            if i % 16 == 0 {
                println!();
            }
            print!("{:02x} ", buffer.add(i).read());
        }
    }
    println!();
    loop {}
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

    let pid = pm.create_kernel_process("kproc", 1, kproc as usize);
    pm.ready(pid);

    // start preemption
    interrupt::timer_interrupt_on();

    pm.schedule();
    loop {
        pm.schedule();
    }
}
