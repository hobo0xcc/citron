use super::paging;
use super::process::TrapFrame;
use crate::arch::syscall::SysCallInfo;
use crate::process::{process_manager, ProcessManager};
use crate::*;

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
        arg as *mut T
    }
}

pub unsafe fn syscall_info() -> RiscvSysCallInfo {
    let pm = process_manager();
    let proc = &mut pm.ptable[pm.running];

    RiscvSysCallInfo {
        trap_frame: proc.arch_proc.trap_frame,
    }
}

pub unsafe fn sys_write(pm: &mut ProcessManager, _fd: usize, buf: *mut u8, count: usize) -> usize {
    let pagetable = pm.ptable[pm.running].arch_proc.page_table.as_mut();
    let buf_phys = paging::virt_to_phys(pagetable, buf as usize).unwrap() as *mut u8;
    for i in 0..count {
        print!("{}", *buf_phys.add(i) as char);
    }

    0
}

pub unsafe fn execute_syscall() -> usize {
    let info = syscall_info();
    let pm = process_manager();
    let syscall_number = info.get_arg_raw(0);
    let ret_val = match syscall_number {
        1 => sys_write(
            pm,
            info.get_arg_raw(1),
            info.get_arg_ptr::<u8>(2),
            info.get_arg_raw(3),
        ),
        _ => unimplemented!(),
    };

    ret_val
}
