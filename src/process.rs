use crate::arch::riscv64::interrupt::interrupt_disable;
use crate::arch::riscv64::interrupt::interrupt_restore;
use crate::arch::target::nullproc;
use crate::arch::target::process::Context;
use crate::arch::target::process::{context_switch, ArchProcess};
use crate::*;
use alloc::alloc::alloc;
use alloc::boxed::Box;
use alloc::collections::binary_heap::BinaryHeap;
use alloc::vec::Vec;
use array_init::array_init;
use core::alloc::Layout;
use core::cmp::Ordering;
use intrusive_collections::intrusive_adapter;
use intrusive_collections::{LinkedList, LinkedListLink};

pub static mut PM: Option<ProcessManager> = None;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum State {
    Running,
    Ready,
    Suspend,
    Sleep,
    Free,
}

pub const KERNEL_STACK_SIZE: usize = 0x2000;

#[derive(Copy, Clone)]
#[allow(dead_code)]
pub struct Process {
    pub state: State,
    pub arch_proc: ArchProcess,
    pub pid: usize,
    pub priority: usize,
    pub name: [u8; 64],
    pub kernel_stack: usize,
    pub user_stack: usize,
}

impl Process {
    pub fn new(pid: usize) -> Self {
        Process {
            state: State::Free,
            arch_proc: ArchProcess::new(pid),
            pid,
            priority: 0,
            name: [0; 64],
            kernel_stack: 0,
            user_stack: 0,
        }
    }
}

#[derive(Clone)]
pub struct ProcessDesc {
    pub priority: usize,
    pub pid: usize,
}

impl ProcessDesc {
    pub fn new(priority: usize, pid: usize) -> Self {
        ProcessDesc { priority, pid }
    }
}

impl Ord for ProcessDesc {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
    }
}

impl PartialOrd for ProcessDesc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ProcessDesc {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for ProcessDesc {}

#[derive(Clone)]
pub struct ProcessDelay {
    pid: usize,
    delay: usize,
    link: LinkedListLink,
}

intrusive_adapter!(ProcessDelayAdapter = Box<ProcessDelay>: ProcessDelay { link: LinkedListLink });

impl ProcessDelay {
    pub fn new(pid: usize, delay: usize) -> Self {
        ProcessDelay {
            pid,
            delay,
            link: LinkedListLink::new(),
        }
    }
}

pub struct DeferScheduler {
    pub count: usize,
    pub attempt: bool,
}

impl DeferScheduler {
    pub fn new() -> Self {
        DeferScheduler {
            count: 0,
            attempt: false,
        }
    }
}

pub enum DeferCommand {
    Start,
    Stop,
}

const PTABLE_SIZE: usize = 16;

#[allow(dead_code)]
pub struct ProcessManager {
    pub ptable: [Process; PTABLE_SIZE],
    pub pqueue: BinaryHeap<ProcessDesc>, // Ready list
    sleep_queue: LinkedList<ProcessDelayAdapter>,
    defer: DeferScheduler,
    pub curr_pid: usize,
    pub running: usize,
}

impl ProcessManager {
    pub fn new() -> Self {
        let mut ptable: [Process; PTABLE_SIZE] = array_init(|i| Process::new(i)); // [Process::new(0); PTABLE_SIZE];
        for (_i, p) in ptable.iter_mut().enumerate() {
            // p.pid = i;
            p.state = State::Free;
        }

        ProcessManager {
            ptable,
            pqueue: BinaryHeap::new(),
            sleep_queue: LinkedList::new(ProcessDelayAdapter::new()),
            defer: DeferScheduler::new(),
            curr_pid: 0,
            running: 0,
        }
    }

    pub fn load_program(&mut self, pid: usize, prog: usize, size: usize) {
        self.ptable[pid].arch_proc.init_program(prog, size);
    }

    pub fn init(&mut self) {
        let pid = self.create_process("null", 0);
        self.load_program(pid, nullproc::null_proc as usize, 0x1000);
        self.ptable[pid].state = State::Running;
        self.running = pid;
    }

    pub fn pop_ready_proc(&mut self) -> Option<ProcessDesc> {
        let proc_desc = &mut self.pqueue.pop();

        while let Some(p) = proc_desc {
            if self.ptable[p.pid].state == State::Ready {
                // println!("{}: {:?}", p.pid, self.ptable[p.pid].state);
                break;
            }

            *proc_desc = self.pqueue.pop();
        }

        (*proc_desc).clone()
    }

    pub fn defer_schedule(&mut self, cmd: DeferCommand) {
        match cmd {
            DeferCommand::Start => {
                if self.defer.count == 0 {
                    self.defer.attempt = false;
                }
                self.defer.count += 1;
            }
            DeferCommand::Stop => {
                self.defer.count -= 1;
                if self.defer.count == 0 && self.defer.attempt {
                    self.schedule();
                }
            }
        }
    }

    pub fn schedule(&mut self) {
        if self.defer.count > 0 {
            self.defer.attempt = true;
            return;
        }

        let proc = self.pop_ready_proc(); // self.pqueue.pop();
        let pdesc = if let Some(desc) = proc {
            desc
        } else {
            return;
        };

        let old_pid = self.running;
        let new_pid = pdesc.pid;
        let old_priority = self.ptable[self.running].priority;
        let new_priority = self.ptable[pdesc.pid].priority;

        let old_context = (&self.ptable[old_pid].arch_proc.context) as *const Context;
        let new_context = (&self.ptable[new_pid].arch_proc.context) as *const Context;
        if old_priority <= new_priority {
            if self.ptable[self.running].state == State::Running {
                self.ptable[self.running].state = State::Ready;
                self.pqueue.push(ProcessDesc::new(old_priority, old_pid));
            }
        } else {
            if self.ptable[self.running].state == State::Running {
                self.pqueue.push(ProcessDesc::new(new_priority, new_pid));
                return;
            }
        }

        self.ptable[new_pid].state = State::Running;
        self.running = new_pid;

        unsafe {
            context_switch(old_context as usize, new_context as usize);
        }
    }

    pub fn ready(&mut self, pid: usize) {
        self.ptable[pid].state = State::Ready;
        let priority = self.ptable[pid].priority;
        self.pqueue.push(ProcessDesc::new(priority, pid));
    }

    pub fn setup_process(&mut self, pid: usize) {
        let kernel_stack = self.ptable[pid].kernel_stack;

        self.ptable[pid].arch_proc = ArchProcess::new(pid);

        self.ptable[pid].arch_proc.init(
            ArchProcess::user_trap_return as usize,
            kernel_stack,
            KERNEL_STACK_SIZE,
        );
    }

    pub fn create_process(&mut self, name: &str, priority: usize) -> usize {
        self.defer_schedule(DeferCommand::Start);
        // search free process entry
        let mut count = 0_usize;
        let mut idx = self.curr_pid;
        loop {
            if count > PTABLE_SIZE {
                panic!("process table exhausted");
            }

            if self.ptable[idx].state == State::Free {
                break;
            }
            count += 1;
            idx += 1;
            idx %= PTABLE_SIZE;
        }

        self.curr_pid = (idx + 1) % PTABLE_SIZE;

        let mut proc = &mut self.ptable[idx];
        proc.priority = priority;
        let name_bytes = name.as_bytes();
        for i in 0_usize..64 {
            if i >= name_bytes.len() {
                proc.name[i] = 0;
            } else {
                proc.name[i] = name_bytes[i];
            }
        }

        let stack_layout = Layout::from_size_align(KERNEL_STACK_SIZE, 0x1000).unwrap();
        let kernel_stack = unsafe { alloc(stack_layout) };
        proc.kernel_stack = kernel_stack as usize;
        proc.state = State::Suspend;

        let pid = proc.pid;
        drop(proc);

        self.setup_process(pid);

        self.defer_schedule(DeferCommand::Stop);

        pid
    }

    pub fn wakeup(&mut self) {
        let mask = interrupt_disable();

        let ptr = self.sleep_queue.pop_front();
        if let Some(mut p) = ptr {
            let proc_delay = p.as_mut();
            if proc_delay.delay >= 1 {
                (*proc_delay).delay -= 1;
            }

            self.sleep_queue.push_front(p);

            let mut cursor = self.sleep_queue.front_mut();
            let mut pids = Vec::new();
            while let Some(p) = cursor.get() {
                if p.delay == 0 {
                    pids.push(p.pid);
                    cursor.remove();
                } else {
                    break;
                }
            }
            drop(cursor);

            for pid in pids.into_iter() {
                if self.ptable[pid].state == State::Sleep {
                    self.ready(pid);
                }
            }
        }

        interrupt_restore(mask);
    }

    pub fn sleep(&mut self, pid: usize, delay: usize) {
        let mask = interrupt_disable();
        self.ptable[pid].state = State::Sleep;

        let mut insert_node = self.sleep_queue.front_mut();
        let mut curr_delay: usize = 0;
        while let Some(node) = insert_node.get() {
            if node.delay > delay {
                break;
            }
            curr_delay = node.delay;

            insert_node.move_next();
        }

        insert_node.insert_before(Box::new(ProcessDelay::new(pid, delay - curr_delay)));

        interrupt_restore(mask);

        self.schedule();
    }

    pub fn kill(&mut self, pid: usize) {
        let mask = interrupt_disable();

        match self.ptable[pid].state {
            State::Free => {}
            _ => {
                self.ptable[pid].state = State::Free;
            }
        }

        self.ptable[pid].arch_proc.free();

        interrupt_restore(mask);

        self.schedule();
    }
}

pub unsafe fn process_manager() -> &'static mut ProcessManager {
    match PM {
        Some(ref mut pm) => &mut *pm,
        None => panic!("process manager is uninitialized"),
    }
}

pub fn init() {
    let mut pm = ProcessManager::new();
    pm.init();
    unsafe {
        PM = Some(pm);
    }
}
