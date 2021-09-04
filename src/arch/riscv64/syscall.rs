use super::paging::*;
use super::process::TrapFrame;
use crate::arch::syscall::SysCallInfo;
use crate::graphics::*;
use crate::process::{process_manager, ProcessManager};
use crate::*;
use alloc::string::*;
use core::slice;

pub struct RiscvSysCallInfo {
    trap_frame: *mut TrapFrame,
}

impl SysCallInfo for RiscvSysCallInfo {
    fn get_arg_raw(&self, idx: usize) -> usize {
        unsafe {
            let arg = match idx {
                0 => (*self.trap_frame).a0,
                1 => (*self.trap_frame).a1,
                2 => (*self.trap_frame).a2,
                3 => (*self.trap_frame).a3,
                4 => (*self.trap_frame).a4,
                5 => (*self.trap_frame).a5,
                6 => (*self.trap_frame).a6,
                7 => (*self.trap_frame).a7,
                _ => panic!("syscall arg number out of index: {}", idx),
            };
            arg
        }
    }

    fn get_arg_ptr<T>(&self, idx: usize) -> *mut T {
        let arg = self.get_arg_raw(idx);
        let pm = unsafe { process_manager() };
        let page_table = unsafe { pm.ptable[pm.running].arch_proc.page_table.as_mut() };
        let ptr = virt_to_phys(page_table, arg).unwrap();
        ptr as *mut T
    }
}

pub unsafe fn syscall_info() -> RiscvSysCallInfo {
    let pm = process_manager();
    let proc = &mut pm.ptable[pm.running];

    RiscvSysCallInfo {
        trap_frame: proc.arch_proc.trap_frame,
    }
}

pub unsafe fn sys_write(_pm: &mut ProcessManager, _fd: usize, buf: *mut u8, count: usize) -> usize {
    // let pagetable = pm.ptable[pm.running].arch_proc.page_table.as_mut();
    // let buf_phys = paging::virt_to_phys(pagetable, buf as usize).unwrap() as *mut u8;
    for i in 0..count {
        print!("{}", buf.add(i).read() as char);
    }

    0
}

pub unsafe fn sys_sleep(pm: &mut ProcessManager, delay: usize) -> usize {
    let pid = pm.ptable[pm.running].pid;
    pm.sleep(pid, delay);

    0
}

pub unsafe fn sys_fork(pm: &mut ProcessManager) -> usize {
    let pid = pm.fork(pm.running);
    let trapframe = pm.ptable[pid].arch_proc.trap_frame;
    // println!("[hobo0xcc] epc: {:#018x}", (*trapframe).epc);
    (*trapframe).a0 = pid;
    (*trapframe).epc += 4;
    pm.ready(pid);

    0
}

pub unsafe fn sys_kill(pm: &mut ProcessManager) -> usize {
    let pid = pm.ptable[pm.running].pid;
    pm.kill(pid);

    0
}

pub unsafe fn sys_create_window(
    _pm: &mut ProcessManager,
    title: *mut u8,
    title_len: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> usize {
    let wm = window_manager();
    let mut title_str = String::new();
    let title_slice = slice::from_raw_parts(title, title_len);
    for ch in title_slice.iter() {
        title_str.push(*ch as char);
    }

    let id = wm.create_window(&title_str, x as u32, y as u32, width as u32, height as u32);
    wm.show_window(id);

    id
}

pub unsafe fn sys_map_window(pm: &mut ProcessManager, window_id: usize, vaddr: usize) -> usize {
    let pid = pm.running;
    let page_table = pm.ptable[pid].arch_proc.page_table.as_mut();

    let arena = object_arena();

    let window = arena.get(window_id);
    let window = if let Some(window) = window {
        (&**window).as_any().downcast_ref::<Window>().unwrap()
    } else {
        return 1;
    };

    let window_frame = window.get_frame();
    let size = window_frame.width * 4 * window_frame.height;
    map_range(
        page_table,
        vaddr,
        window_frame.buffer as usize,
        size as usize,
        EntryBits::R.val() | EntryBits::W.val() | EntryBits::U.val(),
    );

    0
}

pub unsafe fn sys_sync_window(_pm: &mut ProcessManager, window_id: usize) -> usize {
    let wm = window_manager();
    wm.update_window_frame(window_id);

    0
}

pub unsafe fn execute_syscall() -> usize {
    let info = syscall_info();
    let pm = process_manager();
    let syscall_number = info.get_arg_raw(0);
    // println!("[hobo0xcc] syscall: {}", syscall_number);
    let ret_val = match syscall_number {
        1 => sys_write(
            pm,
            info.get_arg_raw(1),
            info.get_arg_ptr::<u8>(2),
            info.get_arg_raw(3),
        ),
        35 => sys_sleep(pm, info.get_arg_raw(1)),
        57 => sys_fork(pm),
        62 => sys_kill(pm),
        1000 => sys_create_window(
            pm,
            info.get_arg_ptr(1),
            info.get_arg_raw(2),
            info.get_arg_raw(3),
            info.get_arg_raw(4),
            info.get_arg_raw(5),
            info.get_arg_raw(6),
        ),
        1001 => sys_map_window(pm, info.get_arg_raw(1), info.get_arg_raw(2)),
        1002 => sys_sync_window(pm, info.get_arg_raw(1)),
        _ => panic!("not implemented: {}", syscall_number),
    };

    ret_val
}
