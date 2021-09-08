use super::csr::Csr;
use super::interrupt;
use super::interrupt::*;
use super::loader::*;
use super::paging;
use super::paging::unmap;
use super::plic;
use super::syscall;
use super::trampoline;
use super::trap;
use super::virtio;
use crate::process::process_manager;
use crate::*;
use alloc::alloc::alloc;
use alloc::alloc::alloc_zeroed;
use alloc::alloc::dealloc;
use core::mem;
use core::ptr::NonNull;
use core::{alloc::Layout, default::Default};

pub const PROC_START: usize = 0x1000;
pub const USER_STACK_START: usize = 0xffff_ffff_ffff_f000;
pub const USER_STACK_SIZE: usize = 0x1000;

#[repr(C)]
#[derive(Copy, Clone)]
#[allow(dead_code)]
pub struct TrapFrame {
    pub kernel_satp: usize,   // 0
    pub kernel_sp: usize,     // 8
    pub kernel_trap: usize,   // 16
    pub epc: usize,           // 24
    pub kernel_hartid: usize, // 32
    pub ra: usize,            // 40
    pub sp: usize,            // 48
    pub gp: usize,            // 56
    pub tp: usize,            // 64
    pub t0: usize,            // 72
    pub t1: usize,            // 80
    pub t2: usize,            // 88
    pub s0: usize,            // 96
    pub s1: usize,            // 104
    pub a0: usize,            // 112
    pub a1: usize,            // 120
    pub a2: usize,            // 128
    pub a3: usize,            // 136
    pub a4: usize,            // 144
    pub a5: usize,            // 152
    pub a6: usize,            // 160
    pub a7: usize,            // 168
    pub s2: usize,            // 176
    pub s3: usize,            // 184
    pub s4: usize,            // 192
    pub s5: usize,            // 200
    pub s6: usize,            // 208
    pub s7: usize,            // 216
    pub s8: usize,            // 224
    pub s9: usize,            // 232
    pub s10: usize,           // 240
    pub s11: usize,           // 248
    pub t3: usize,            // 256
    pub t4: usize,            // 264
    pub t5: usize,            // 272
    pub t6: usize,            // 280
    pub arch_proc: usize,     // 288
}

impl Default for TrapFrame {
    fn default() -> Self {
        TrapFrame {
            kernel_satp: 0,
            kernel_sp: 0,
            kernel_trap: 0,
            kernel_hartid: 0,
            epc: 0,
            ra: 0,
            sp: 0,
            gp: 0,
            tp: 0,
            t0: 0,
            t1: 0,
            t2: 0,
            s0: 0,
            s1: 0,
            a0: 0,
            a1: 0,
            a2: 0,
            a3: 0,
            a4: 0,
            a5: 0,
            a6: 0,
            a7: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
            t3: 0,
            t4: 0,
            t5: 0,
            t6: 0,
            arch_proc: 0,
        }
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[allow(dead_code)]
pub struct Context {
    pub ra: usize,
    pub sp: usize,
    pub s0: usize,
    pub s1: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
    pub a0: usize,
    pub sstatus: usize,
}

impl Default for Context {
    fn default() -> Self {
        Context {
            ra: 0,
            sp: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
            a0: 0,
            sstatus: 0,
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ArchProcess {
    pub page_table: NonNull<paging::Table>,
    pub trap_frame: *mut TrapFrame,
    pub context: Context,
    pub kernel_stack: usize,
    pub kernel_stack_size: usize,
    pub user_stack: usize,
    pub user_stack_size: usize,
    pub exec_info: ExecutableInfo,
    pub pid: usize,
}

extern "C" {
    pub fn context_switch(old_context: usize, new_context: usize);
}

global_asm!(
    ".globl context_switch",
    "context_switch:",
    "sd ra, 0(a0)",
    "sd sp, 8(a0)",
    "sd s0, 16(a0)",
    "sd s1, 24(a0)",
    "sd s2, 32(a0)",
    "sd s3, 40(a0)",
    "sd s4, 48(a0)",
    "sd s5, 56(a0)",
    "sd s6, 64(a0)",
    "sd s7, 72(a0)",
    "sd s8, 80(a0)",
    "sd s9, 88(a0)",
    "sd s10, 96(a0)",
    "sd s11, 104(a0)",
    "csrr t0, sstatus",
    "sd t0, 120(a0)",
    "ld ra,  0(a1)",
    "ld sp,  8(a1)",
    "ld s0,  16(a1)",
    "ld s1,  24(a1)",
    "ld s2,  32(a1)",
    "ld s3,  40(a1)",
    "ld s4,  48(a1)",
    "ld s5,  56(a1)",
    "ld s6,  64(a1)",
    "ld s7,  72(a1)",
    "ld s8,  80(a1)",
    "ld s9,  88(a1)",
    "ld s10, 96(a1)",
    "ld s11, 104(a1)",
    // for first argument of user_trap_return
    "ld a0, 112(a1)",
    "ld t0, 120(a1)",
    "csrw sstatus, t0",
    "ret"
);

impl ArchProcess {
    pub fn new(pid: usize) -> Self {
        let layout = Layout::from_size_align(0x1000, 0x1000).unwrap();
        // let page_table = unsafe { alloc_zeroed(layout) } as *mut paging::Table;
        let trap_frame = unsafe { alloc_zeroed(layout) } as *mut TrapFrame;
        ArchProcess {
            page_table: NonNull::dangling(),
            trap_frame,
            context: Default::default(),
            kernel_stack: 0,
            kernel_stack_size: 0,
            user_stack: 0,
            user_stack_size: 0,
            exec_info: ExecutableInfo::new(),
            pid,
        }
    }

    pub fn free(&mut self) {
        let user_stack_layout = Layout::from_size_align(self.user_stack_size, 0x1000).unwrap();
        let page_table_layout = Layout::from_size_align(0x1000, 0x1000).unwrap();
        let trap_frame_layout = Layout::from_size_align(0x1000, 0x1000).unwrap();

        unsafe {
            dealloc(self.user_stack as *mut u8, user_stack_layout);
            unmap(self.page_table.as_mut());
            dealloc(self.page_table.as_ptr() as *mut u8, page_table_layout);
            dealloc(self.trap_frame as *mut u8, trap_frame_layout);

            for segment in self.exec_info.segment_buffers.iter() {
                dealloc(segment.ptr, segment.layout);
            }
        }
    }

    pub fn init_program(&mut self, path: &str) {
        let exec_info = unsafe { load_exe(path, self.page_table.as_mut()).unwrap() };

        self.exec_info = exec_info;

        unsafe {
            (*self.trap_frame).epc = self.exec_info.entry;
        }
    }

    pub fn init(&mut self, start: usize, kernel_stack: usize, kernel_stack_size: usize) {
        self.kernel_stack = kernel_stack;
        self.kernel_stack_size = kernel_stack_size;
        self.setup_pagetable();
        self.init_context(start, kernel_stack + kernel_stack_size);

        unsafe {
            // (*self.trap_frame).epc = PROC_START;
            (*self.trap_frame).sp = USER_STACK_START;
            (*self.trap_frame).ra = trampoline::KILLME;
        }
    }

    pub fn interrupt(&mut self, code: usize) {
        match code {
            1 => {
                // supervisor software interrupt
                let pm = unsafe { process_manager() };
                Csr::Sip.write(Csr::Sip.read() & !(1 << 1)); // clear SSIP
                pm.wakeup().expect("process");
                pm.schedule().expect("process");
            }
            9 => {
                let irq = plic::claim();
                if (irq as usize) >= plic::Irq::VirtioFirstIrq.val()
                    && (irq as usize) <= plic::Irq::VirtioEndIrq.val()
                {
                    virtio::interrupt(irq);
                } else if (irq as usize) == plic::Irq::UartIrq.val() {
                    // TODO: uart interrupt handler
                }

                if irq != 0 {
                    plic::complete(irq as u32);
                }
            }
            _ => {
                println!("unknown interrupt: {}", code);
            }
        }
    }

    pub fn exception(&mut self, code: usize) {
        if code == 8 {
            interrupt_on();
            // system call
            let ret_val = unsafe { syscall::execute_syscall() };
            unsafe {
                (*self.trap_frame).a0 = ret_val;
                (*self.trap_frame).epc += 4;
            }
        } else {
            let sepc = Csr::Sepc.read();
            let stval = Csr::Stval.read();
            println!("==exception occurred==");
            println!("scause : {:#018x}", code);
            println!("sepc   : {:#018x}", sepc);
            println!("stval  : {:#018x}", stval);
            loop {}
        }
    }

    pub unsafe extern "C" fn user_trap(&mut self) {
        let sstatus = Csr::Sstatus.read();
        if sstatus & (1 << 8) != 0 {
            // SPP isn't user
            panic!("not from user mode");
        }

        (*self.trap_frame).epc = Csr::Sepc.read();

        Csr::Stvec.write(trap::kernelvec as usize);

        let scause = Csr::Scause.read();
        if scause & (1 << 63) != 0 {
            self.interrupt(scause & !(1 << 63));
        } else {
            self.exception(scause);
        }

        self.user_trap_return();
    }

    pub unsafe extern "C" fn user_trap_return(&mut self) {
        interrupt::interrupt_off();

        let stvec = trampoline::TRAMPOLINE
            + (trampoline::uservec as usize - trampoline::trampoline as usize);
        Csr::Stvec.write(stvec);

        let satp_val = Csr::Satp.read();
        // let page_table = ((satp_val & !(8 << 60)) << 12) as *mut paging::Table;
        (*self.trap_frame).kernel_satp = satp_val;
        (*self.trap_frame).kernel_sp = self.kernel_stack + self.kernel_stack_size;
        (*self.trap_frame).kernel_trap = Self::user_trap as usize;

        let mut sstatus = Csr::Sstatus.read();
        sstatus &= !(1 << 8); // unset SSTATUS.SPP (user mode)
        sstatus |= 1 << 5; // set SSTATUS.SPIE (enable interrupt in user mode)
        Csr::Sstatus.write(sstatus);

        let sepc_val = (*self.trap_frame).epc;
        Csr::Sepc.write(sepc_val);

        (*self.trap_frame).arch_proc = self as *mut ArchProcess as usize;

        #[cfg(target_pointer_width = "32")]
        let mut satp = 1_usize << 31 | (self.page_table as usize >> 12);

        #[cfg(target_pointer_width = "64")]
        let satp = 8_usize << 60 | (self.page_table.as_ptr() as usize >> 12);
        let func_usize: usize = trampoline::TRAMPOLINE
            + (trampoline::userret as usize - trampoline::trampoline as usize);

        let func = mem::transmute::<usize, fn(usize, usize)>(func_usize);
        // jump to userret
        func(trampoline::TRAPFRAME, satp);
    }

    pub fn setup_pagetable(&mut self) {
        let layout = Layout::from_size_align(0x1000, 0x1000).unwrap();
        let page_table = unsafe { alloc_zeroed(layout) } as *mut paging::Table;
        self.page_table = NonNull::new(page_table).unwrap();
        unsafe {
            paging::map(
                self.page_table.as_mut(),
                trampoline::TRAMPOLINE,
                trampoline::trampoline as usize,
                paging::EntryBits::R.val() | paging::EntryBits::X.val(),
                0,
            );
            paging::map(
                self.page_table.as_mut(),
                trampoline::TRAPFRAME,
                self.trap_frame as usize,
                paging::EntryBits::R.val() | paging::EntryBits::W.val(),
                0,
            );
            paging::map(
                self.page_table.as_mut(),
                trampoline::KILLME,
                trampoline::killme as usize,
                paging::EntryBits::R.val()
                    | paging::EntryBits::X.val()
                    | paging::EntryBits::U.val(),
                0,
            );

            let stack_layout = Layout::from_size_align(USER_STACK_SIZE, 0x1000).unwrap();
            let user_stack = alloc(stack_layout) as usize;
            self.user_stack = user_stack;
            self.user_stack_size = USER_STACK_SIZE;
            paging::map(
                self.page_table.as_mut(),
                USER_STACK_START - USER_STACK_SIZE,
                user_stack,
                paging::EntryBits::R.val()
                    | paging::EntryBits::W.val()
                    | paging::EntryBits::U.val(),
                0,
            );
        }
    }

    pub fn init_context(&mut self, start: usize, stack: usize) {
        self.context.ra = start;
        self.context.sp = stack;
        self.context.a0 = self as *mut ArchProcess as usize;
        self.context.sstatus = Csr::Sstatus.read();
    }
}
