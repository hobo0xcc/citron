use crate::arch::target::interrupt;
use crate::*;

// kernel process
#[no_mangle]
pub extern "C" fn kproc() {
    let gpu = unsafe { arch::target::virtio::gpu_device() };
    gpu.init_display();
    let width = gpu.get_width();
    let height = gpu.get_height();
    let framebuffer = gpu.get_framebuffer() as *mut u32;
    for x in 0..width {
        for y in 0..height {
            unsafe {
                let idx = y * width + x;
                framebuffer.add(idx as usize).write(x * y);
            }
        }
    }
    gpu.update_display();
    // let blk = unsafe { arch::target::virtio::block_device() };
    // let layout = Layout::from_size_align(512, 512).unwrap();
    // let buffer = unsafe { slice::from_raw_parts_mut(alloc_zeroed(layout), 512) };
    // let _ = blk.read_sector(0, buffer);
    // println!("kernel.elf:");
    // for i in 0..512 {
    //     if i % 16 == 0 {
    //         println!();
    //     }
    //     print!("{:02x} ", buffer[i]);
    // }
    // println!();
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
    interrupt::interrupt_on();

    pm.schedule();
    loop {
        pm.schedule();
    }
}
