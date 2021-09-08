use crate::arch::riscv64::trampoline;
use crate::arch::target::interrupt::interrupt_disable;
use crate::arch::target::interrupt::interrupt_restore;
use crate::arch::target::loader::ExecutableInfo;
use crate::arch::target::loader::*;
use crate::arch::target::paging::*;
use crate::arch::target::process::*;
use crate::*;
use alloc::alloc::alloc;
use alloc::alloc::alloc_zeroed;
use alloc::alloc::dealloc;
use alloc::boxed::Box;
use alloc::collections::binary_heap::BinaryHeap;
use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;
use array_init::array_init;
use core::alloc::Layout;
use core::cmp::Ordering;
use core::ptr::NonNull;
use hashbrown::HashMap;
use intrusive_collections::intrusive_adapter;
use intrusive_collections::{LinkedList, LinkedListLink};

pub static mut PM: Option<ProcessManager> = None;

#[derive(Copy, Clone, PartialEq, Debug, Hash, Eq)]
pub enum ProcessEvent {
    MouseEvent,
    KeyboardEvent,
    Exit(usize),
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum State {
    Running,
    Ready,
    Suspend,
    Sleep,
    SemaWait,
    IOWait,
    EventWait,
    Free,
}

pub const KERNEL_STACK_SIZE: usize = 0x10000;

#[derive(Clone)]
#[allow(dead_code)]
pub struct Process {
    pub state: State,
    pub arch_proc: ArchProcess,
    pub pid: usize,
    pub priority: usize,
    children: VecDeque<usize>,
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
            children: VecDeque::new(),
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

#[derive(Eq, PartialEq)]
pub enum SemaState {
    Used,
    Free,
}

pub struct Semaphore {
    state: SemaState,
    count: isize,
    queue: VecDeque<usize>, // wait process queue
}

impl Semaphore {
    pub fn new(count: isize) -> Self {
        Semaphore {
            state: SemaState::Free,
            count,
            queue: VecDeque::new(),
        }
    }
}

const PTABLE_SIZE: usize = 16;
const STABLE_SIZE: usize = 16;

#[allow(dead_code)]
pub struct ProcessManager {
    pub ptable: [Process; PTABLE_SIZE],
    pub stable: [Semaphore; STABLE_SIZE],
    pub pqueue: BinaryHeap<ProcessDesc>, // Ready list
    sleep_queue: LinkedList<ProcessDelayAdapter>,
    event_queue: HashMap<ProcessEvent, Vec<usize>>,
    defer: DeferScheduler,
    pub curr_pid: usize,
    pub running: usize,
}

impl ProcessManager {
    pub fn new() -> Self {
        let ptable: [Process; PTABLE_SIZE] = array_init(|i| Process::new(i)); // [Process::new(0); PTABLE_SIZE];
        let stable: [Semaphore; STABLE_SIZE] = array_init(|_i| Semaphore::new(1));

        ProcessManager {
            ptable,
            stable,
            pqueue: BinaryHeap::new(),
            sleep_queue: LinkedList::new(ProcessDelayAdapter::new()),
            event_queue: HashMap::new(),
            defer: DeferScheduler::new(),
            curr_pid: 0,
            running: 0,
        }
    }

    pub fn load_program(&mut self, pid: usize, path: &str) {
        self.ptable[pid].arch_proc.init_program(path);
    }

    pub fn init(&mut self) {
        let pid = self.create_process("null", 0, true);
        // self.load_program(pid, nullproc::null_proc as usize, 0x1000);
        self.ptable[pid].state = State::Running;
        self.running = pid;
    }

    pub fn create_semaphore(&mut self, count: isize) -> usize {
        let mask = interrupt_disable();

        for (i, sema) in self.stable.iter_mut().enumerate() {
            if sema.state == SemaState::Free {
                sema.state = SemaState::Used;
                sema.count = count;
                interrupt_restore(mask);
                return i;
            }
        }

        interrupt_restore(mask);
        panic!("semaphore exhausted");
    }

    pub fn wait_semaphore(&mut self, sid: usize) {
        let mask = interrupt_disable();

        let pid = self.running;
        let sema = &mut self.stable[sid];
        sema.count -= 1;
        if sema.count < 0 {
            self.ptable[pid].state = State::SemaWait;
            sema.queue.push_back(pid);
            self.schedule();
        }

        interrupt_restore(mask);
    }

    pub fn signal_semaphore(&mut self, sid: usize) {
        let mask = interrupt_disable();

        if self.stable[sid].count < 0 {
            let pid = self.stable[sid].queue.pop_front().unwrap();
            self.stable[sid].count += 1;
            self.ready(pid);
        } else {
            self.stable[sid].count += 1;
        }

        interrupt_restore(mask);
    }

    pub fn delete_semaphore(&mut self, sid: usize) {
        let mask = interrupt_disable();

        self.stable[sid].state = SemaState::Free;

        self.defer_schedule(DeferCommand::Start);

        while self.stable[sid].count < 0 {
            let pid = self.stable[sid].queue.pop_front().unwrap();
            self.ready(pid);
            self.stable[sid].count += 1;
        }

        self.defer_schedule(DeferCommand::Stop);

        self.schedule();

        interrupt_restore(mask);
    }

    pub fn pop_ready_proc(&mut self) -> Option<ProcessDesc> {
        let proc_desc = &mut self.pqueue.pop();

        while let Some(p) = proc_desc {
            if self.ptable[p.pid].state == State::Ready {
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
        let mask = interrupt_disable();

        if self.defer.count > 0 {
            self.defer.attempt = true;
            interrupt_restore(mask);
            return;
        }

        let proc = self.pop_ready_proc(); // self.pqueue.pop();
        let pdesc = if let Some(desc) = proc {
            desc
        } else {
            interrupt_restore(mask);
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
                interrupt_restore(mask);
                return;
            }
        }

        self.ptable[new_pid].state = State::Running;
        self.running = new_pid;

        // println!("interrupt: {}", is_interrupt_enable());

        println!("[hobo0xcc] switch: {} -> {}", old_pid, new_pid);
        // unsafe {
        //     println!("[hobo0xcc] context.ra: {:#018x}", (*new_context).ra);
        // }
        // let mut sp: usize = 0;
        // unsafe {
        //     asm!("mv {}, sp", out(reg)sp);
        // }
        // println!("sp: {:#018x}", sp);

        unsafe {
            context_switch(old_context as usize, new_context as usize);
        }
        interrupt_restore(mask);
    }

    pub fn ready(&mut self, pid: usize) {
        if self.ptable[pid].state == State::Free
            || self.ptable[pid].state == State::Running
            || self.ptable[pid].state == State::Ready
        {
            return;
        }
        self.ptable[pid].state = State::Ready;
        let priority = self.ptable[pid].priority;
        self.pqueue.push(ProcessDesc::new(priority, pid));
        self.schedule();
    }

    pub fn setup_kernel_process(&mut self, pid: usize, func: usize) {
        let kernel_stack = self.ptable[pid].kernel_stack;

        self.ptable[pid].arch_proc = ArchProcess::new(pid);

        self.ptable[pid]
            .arch_proc
            .init(func, kernel_stack, KERNEL_STACK_SIZE);
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

    pub fn create_kernel_process(&mut self, name: &str, priority: usize, func: usize) -> usize {
        // let mask = interrupt_disable();

        let pid = self.create_process(name, priority, false);
        self.setup_kernel_process(pid, func);

        // interrupt_restore(mask);

        pid
    }

    pub fn create_process(&mut self, name: &str, priority: usize, do_setup: bool) -> usize {
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

        if do_setup {
            self.setup_process(pid);
        }

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

            self.defer_schedule(DeferCommand::Start);
            for pid in pids.into_iter() {
                if self.ptable[pid].state == State::Sleep {
                    self.ready(pid);
                }
            }
            self.defer_schedule(DeferCommand::Stop);

            self.schedule();
        }

        interrupt_restore(mask);
    }

    pub fn sleep(&mut self, pid: usize, delay: usize) {
        let mask = interrupt_disable();
        self.ptable[pid].state = State::Sleep;

        let mut insert_node = self.sleep_queue.front_mut();
        let mut delay_sum: usize = 0;
        while let Some(node) = insert_node.get() {
            if delay < (delay_sum + node.delay) {
                break;
            }
            delay_sum += node.delay;

            insert_node.move_next();
        }

        let relative_delay = delay - delay_sum;
        insert_node.insert_before(Box::new(ProcessDelay::new(pid, relative_delay)));
        if let Some(node) = insert_node.get() {
            let pid = node.pid;
            let delay = node.delay;
            drop(node);
            let _ =
                insert_node.replace_with(Box::new(ProcessDelay::new(pid, delay - relative_delay)));
        }

        self.schedule();

        interrupt_restore(mask);
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
        let layout = Layout::from_size_align(KERNEL_STACK_SIZE, 0x1000).unwrap();
        unsafe {
            dealloc(self.ptable[pid].kernel_stack as *mut u8, layout);
        }

        self.event_signal(ProcessEvent::Exit(pid));

        self.schedule();

        interrupt_restore(mask);
    }

    pub fn io_wait(&mut self, pid: usize) {
        let mask = interrupt_disable();

        self.ptable[pid].state = State::IOWait;

        interrupt_restore(mask);
    }

    pub fn io_signal(&mut self, pid: usize) {
        let mask = interrupt_disable();

        self.ready(pid);

        interrupt_restore(mask);
    }

    pub fn event_wait(&mut self, pid: usize, event: ProcessEvent) {
        let mask = interrupt_disable();

        self.ptable[pid].state = State::EventWait;
        self.event_queue
            .entry(event)
            .or_insert_with(|| vec![])
            .push(pid);

        self.schedule();

        interrupt_restore(mask);
    }

    pub fn event_signal(&mut self, event: ProcessEvent) {
        let mask = interrupt_disable();

        self.defer_schedule(DeferCommand::Start);
        let events = self.event_queue.remove(&event);
        let events = if let Some(events) = events {
            events
        } else {
            self.defer_schedule(DeferCommand::Stop);
            interrupt_restore(mask);
            return;
        };
        for pid in events.iter() {
            self.ready(*pid);
        }
        self.defer_schedule(DeferCommand::Stop);

        self.schedule();

        interrupt_restore(mask);
    }

    pub fn wait_exit(&mut self) {
        let mask = interrupt_disable();

        for pid in self.ptable[self.running].children.clone() {
            if self.ptable[pid].state == State::Free {
                self.ptable[self.running].children.remove(pid);
                return;
            }
        }

        self.defer_schedule(DeferCommand::Start);
        for pid in self.ptable[self.running].children.clone() {
            self.event_wait(self.running, ProcessEvent::Exit(pid));
        }
        self.defer_schedule(DeferCommand::Stop);

        self.schedule();

        interrupt_restore(mask);
    }

    pub fn fork(&mut self, pid: usize) -> usize {
        let mask = interrupt_disable();

        let new_pid = self.curr_pid;
        self.curr_pid += 1;
        let mut new_proc = Process::new(new_pid);

        self.ptable[pid].children.push_back(new_pid);

        new_proc.pid = new_pid;
        new_proc.state = State::Suspend;

        let layout = Layout::from_size_align(0x1000, 0x1000).unwrap();
        let new_trapframe = unsafe { alloc_zeroed(layout) };
        unsafe {
            new_trapframe.copy_from_nonoverlapping(
                self.ptable[pid].arch_proc.trap_frame as *const u8,
                0x1000,
            );
            new_proc.arch_proc.trap_frame = new_trapframe as *mut TrapFrame;
        }

        let layout =
            Layout::from_size_align(self.ptable[pid].arch_proc.kernel_stack_size, 0x1000).unwrap();
        let kernel_stack = unsafe { alloc_zeroed(layout) };
        unsafe {
            kernel_stack.copy_from_nonoverlapping(
                self.ptable[pid].arch_proc.kernel_stack as *const u8,
                self.ptable[pid].arch_proc.kernel_stack_size,
            );
        }

        let layout =
            Layout::from_size_align(self.ptable[pid].arch_proc.user_stack_size, 0x1000).unwrap();
        let user_stack = unsafe { alloc_zeroed(layout) };
        unsafe {
            user_stack.copy_from_nonoverlapping(
                self.ptable[pid].arch_proc.user_stack as *const u8,
                self.ptable[pid].arch_proc.user_stack_size,
            );
        }

        unsafe {
            new_proc.arch_proc.page_table =
                NonNull::new(self.ptable[pid].arch_proc.page_table.as_mut().clone()).unwrap();
        }

        new_proc.arch_proc.kernel_stack = kernel_stack as usize;
        new_proc.kernel_stack = kernel_stack as usize;
        new_proc.arch_proc.init_context(
            ArchProcess::user_trap_return as usize,
            self.ptable[pid].arch_proc.kernel_stack + self.ptable[pid].arch_proc.kernel_stack_size,
        );
        new_proc.arch_proc.user_stack = user_stack as usize;

        unsafe {
            map(
                new_proc.arch_proc.page_table.as_mut(),
                USER_STACK_START - USER_STACK_SIZE,
                new_proc.arch_proc.user_stack,
                EntryBits::R.val() | EntryBits::W.val() | EntryBits::U.val(),
                0,
            );
            map(
                new_proc.arch_proc.page_table.as_mut(),
                trampoline::TRAPFRAME,
                new_proc.arch_proc.trap_frame as usize,
                EntryBits::R.val() | EntryBits::W.val(),
                0,
            );
        }

        let mut new_exec_info = ExecutableInfo {
            entry: self.ptable[pid].arch_proc.exec_info.entry,
            segment_buffers: Vec::new(),
        };
        for segment in self.ptable[pid].arch_proc.exec_info.segment_buffers.iter() {
            let new_layout = segment.layout.clone();
            let new_segment = unsafe { alloc_zeroed(new_layout) };
            let vm_range = segment.vm_range.clone();
            let flags = segment.flags;
            unsafe {
                new_segment.copy_from_nonoverlapping(segment.ptr, new_layout.size());
                map_range(
                    new_proc.arch_proc.page_table.as_mut(),
                    vm_range.start,
                    new_segment as usize,
                    vm_range.len(),
                    flags,
                );
            }

            new_exec_info.segment_buffers.push(Segment::new(
                new_segment,
                new_layout,
                vm_range,
                flags,
            ));
        }

        new_proc.arch_proc.exec_info = new_exec_info;

        self.ptable[new_pid] = new_proc;

        self.ptable[new_pid].priority = 1;

        interrupt_restore(mask);

        new_pid
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
