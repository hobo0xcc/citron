use crate::arch::target::interrupt;
use crate::*;
use core::slice;
use tiny_skia::*;

// kernel process
#[no_mangle]
pub extern "C" fn kproc() {
    let gpu = unsafe { arch::target::virtio::gpu_device() };
    gpu.init_display();
    let width = gpu.get_width();
    let height = gpu.get_height();
    let framebuffer = gpu.get_framebuffer();
    let framebuffer32 = framebuffer as *mut u32;
    for x in 0..width {
        for y in 0..height {
            unsafe {
                let idx = y * width + x;
                framebuffer32.add(idx as usize).write(0xffffffff);
            }
        }
    }
    gpu.update_display();

    let mut paint1 = Paint::default();
    paint1.set_color_rgba8(50, 127, 150, 200);
    paint1.anti_alias = true;

    let mut paint2 = Paint::default();
    paint2.set_color_rgba8(220, 140, 75, 180);

    let path1 = {
        let mut pb = PathBuilder::new();
        pb.move_to(60.0, 60.0);
        pb.line_to(160.0, 940.0);
        pb.cubic_to(380.0, 840.0, 660.0, 800.0, 940.0, 800.0);
        pb.cubic_to(740.0, 460.0, 440.0, 160.0, 60.0, 60.0);
        pb.close();
        pb.finish().unwrap()
    };

    let path2 = {
        let mut pb = PathBuilder::new();
        pb.move_to(940.0, 60.0);
        pb.line_to(840.0, 940.0);
        pb.cubic_to(620.0, 840.0, 340.0, 800.0, 60.0, 800.0);
        pb.cubic_to(260.0, 460.0, 560.0, 160.0, 940.0, 60.0);
        pb.close();
        pb.finish().unwrap()
    };

    let framebuffer_slice =
        unsafe { slice::from_raw_parts_mut(framebuffer, (width * height * 4) as usize) };

    let mut pixmap = PixmapMut::from_bytes(framebuffer_slice, width, height).unwrap();
    pixmap.fill_path(
        &path1,
        &paint1,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    pixmap.fill_path(
        &path2,
        &paint2,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

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
