use crate::arch::riscv64::virtio::gpu_device;
use crate::arch::riscv64::virtio::keyboard_device;
use crate::arch::riscv64::virtio::mouse_device;
use crate::arch::target::interrupt;
use crate::arch::target::virtio::virtio_input::*;
use crate::graphics::layer_manager;
use crate::graphics::*;
use crate::process::*;
use crate::*;
use core::arch::asm;

pub unsafe extern "C" fn kproc() {
    let pm = process_manager();

    let mouse = mouse_device();
    let keyboard = keyboard_device();
    let gpu = gpu_device();
    mouse.lock().init_input_event();
    keyboard.lock().init_input_event();
    gpu.lock().init_display();
    graphics::init();

    let lm = layer_manager();

    // let mouse = mouse_device();

    loop {
        let queue = &mut mouse.lock().event_queue; //mouse_event_queue();
        if queue.is_empty() {
            pm.event_wait(pm.running, ProcessEvent::MouseEvent)
                .expect("process");
            continue;
        }

        let mut event = queue.pop_front();
        while let Some(ev) = event {
            match EventType::from(ev.type_) {
                EventType::EV_REL => {
                    if ev.code == EV_REL::REL_X as u16 {
                        lm.move_rel(MOUSE_LAYER_ID, ev.value as i32, 0);
                    } else if ev.code == EV_REL::REL_Y as u16 {
                        lm.move_rel(MOUSE_LAYER_ID, 0, ev.value as i32);
                    }
                }
                EventType::EV_KEY => {
                    if ev.code == EV_KEY::BTN_LEFT as u16 && ev.value == 1 {
                        let x = lm.get_layer_x(MOUSE_LAYER_ID);
                        let y = lm.get_layer_y(MOUSE_LAYER_ID);
                        lm.on_event(ObjectEvent::MouseLeftPress(x, y), MOUSE_LAYER_ID);
                    } else if ev.code == EV_KEY::BTN_LEFT as u16 && ev.value == 0 {
                        let x = lm.get_layer_x(MOUSE_LAYER_ID);
                        let y = lm.get_layer_y(MOUSE_LAYER_ID);
                        lm.on_event(ObjectEvent::MouseLeftRelease(x, y), MOUSE_LAYER_ID);
                    }
                }
                EventType::EV_SYN => {
                    lm.update(MOUSE_LAYER_ID);
                }
                _ => {}
            }
            event = queue.pop_front();
        }
    }
}

pub unsafe extern "C" fn fs_proc() {
    fs::fat::init();
    fs::init();
    let pm = process_manager();
    // pm.defer_schedule(DeferCommand::Start).unwrap();
    let pid = pm.create_process("mandelbrot", 1, true).unwrap();
    pm.load_program(pid, "/bin/mandelbrot").unwrap();
    pm.ready(pid).unwrap();

    let pid = pm.create_process("main", 1, true).unwrap();
    pm.load_program(pid, "/bin/main").unwrap();
    pm.ready(pid).unwrap();
    // pm.defer_schedule(DeferCommand::Stop).unwrap();

    // // pm.kill(pm.running);
    // let running = pm.running;
    // get_process_mut!(pm.ptable_lock_mut(), running)
    //     .expect("process")
    //     .state = State::Free;
    // pm.schedule().expect("process");

    loop {
        pm.schedule().expect("process");
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

    unsafe {
        init::init_all();
    }

    println!("Initialization done");
    println!("Hello, citron!");

    let pm = unsafe { process::process_manager() };

    // start preemption
    interrupt::timer_interrupt_off();
    interrupt::interrupt_on();

    pm.defer_schedule(DeferCommand::Start).expect("process");

    let pid = pm
        .create_kernel_process("fs", 1, fs_proc as usize)
        .expect("process");
    pm.ready(pid).expect("process");

    let pid = pm
        .create_kernel_process("kproc", 2, kproc as usize)
        .expect("process");
    pm.ready(pid).expect("process");

    pm.defer_schedule(DeferCommand::Stop).expect("process");

    interrupt::timer_interrupt_on();

    loop {
        pm.schedule().expect("process");
    }
}
