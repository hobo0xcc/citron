use crate::arch::riscv64::virtio::gpu_device;
use crate::arch::riscv64::virtio::keyboard_device;
use crate::arch::riscv64::virtio::mouse_device;
use crate::arch::target::interrupt;
use crate::arch::target::virtio::virtio_input::*;
use crate::graphics::layer_manager;
use crate::graphics::*;
use crate::*;

pub unsafe fn kproc() {
    let gpu = gpu_device();
    gpu.init_display();
    let mouse = mouse_device();
    mouse.init_input_event();
    let keyboard = keyboard_device();
    keyboard.init_input_event();
    graphics::init();
    let lm = layer_manager();

    loop {
        if mouse.event_queue.is_empty() {
            continue;
        }

        let mut event = mouse.event_queue.pop_front();
        while let Some(ev) = event {
            match EventType::from(ev.type_) {
                EventType::EV_REL => {
                    if ev.code == EV_REL::REL_X as u16 {
                        lm.move_rel(MOUSE_LAYER_ID, ev.value as i32, 0);
                    } else if ev.code == EV_REL::REL_Y as u16 {
                        lm.move_rel(MOUSE_LAYER_ID, 0, ev.value as i32);
                    }
                }
                EventType::EV_SYN => {
                    lm.update(MOUSE_LAYER_ID);
                }
                _ => {}
            }
            event = mouse.event_queue.pop_front();
        }
    }
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
    //interrupt::timer_interrupt_on();
    interrupt::interrupt_on();

    pm.schedule();
    loop {
        pm.schedule();
    }
}
