use crate::arch::target::interrupt::interrupt_disable;
use crate::arch::target::interrupt::interrupt_restore;
use crate::arch::target::process::*;
use crate::spinlock::*;
use crate::*;
use alloc::alloc::alloc;
use alloc::alloc::dealloc;
use alloc::boxed::Box;
use alloc::collections::binary_heap::BinaryHeap;
use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cmp::Ordering;
use core::marker::PhantomData;
use hashbrown::HashMap;
use intrusive_collections::intrusive_adapter;
use intrusive_collections::{LinkedList, LinkedListLink};

pub type Pid = usize;
pub type Sid = usize;

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
    pub pid: Pid,
    pub priority: usize,
    children: VecDeque<Pid>,
    pub name: String,
    pub kernel_stack: usize,
    pub user_stack: usize,
}

impl Process {
    pub fn new(pid: Pid) -> Self {
        Process {
            state: State::Free,
            arch_proc: ArchProcess::new(pid),
            pid,
            priority: 0,
            children: VecDeque::new(),
            name: String::new(),
            kernel_stack: 0,
            user_stack: 0,
        }
    }
}

#[derive(Clone)]
pub struct ProcessDesc {
    pub priority: usize,
    pub pid: Pid,
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
    pid: Pid,
    delay: usize,
    link: LinkedListLink,
}

intrusive_adapter!(ProcessDelayAdapter = Box<ProcessDelay>: ProcessDelay { link: LinkedListLink });

impl ProcessDelay {
    pub fn new(pid: Pid, delay: usize) -> Self {
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

#[derive(Copy, Clone, Debug)]
pub enum ProcessError {
    ProcessNotFound(Pid),
    SemaphoreNotFound(Sid),
}

#[macro_export]
macro_rules! get_process_mut {
    ($ptable: expr, $pid: expr) => {
        $ptable
            .get_mut(&$pid)
            .ok_or(ProcessError::ProcessNotFound($pid))
    };
}

#[macro_export]
macro_rules! get_process {
    ($ptable: expr, $pid: expr) => {
        $ptable
            .get(&$pid)
            .ok_or(ProcessError::ProcessNotFound($pid))
    };
}

#[allow(dead_code)]
pub struct ProcessManager<'a> {
    pub ptable: SpinLock<BTreeMap<Pid, Process>>,
    pub stable: SpinLock<BTreeMap<Sid, Semaphore>>,
    pub pqueue: SpinLock<BinaryHeap<ProcessDesc>>, // Ready list
    sleep_queue: SpinLock<LinkedList<ProcessDelayAdapter>>,
    event_queue: HashMap<ProcessEvent, Vec<Pid>>,
    defer: DeferScheduler,
    pub curr_pid: Pid,
    pub curr_sid: Sid,
    pub running: Pid,
    phantom: PhantomData<&'a u8>,
}

impl<'a> ProcessManager<'a> {
    pub fn new() -> Self {
        ProcessManager {
            ptable: SpinLock::new(BTreeMap::new()),
            stable: SpinLock::new(BTreeMap::new()),
            pqueue: SpinLock::new(BinaryHeap::new()),
            sleep_queue: SpinLock::new(LinkedList::new(ProcessDelayAdapter::new())),
            event_queue: HashMap::new(),
            defer: DeferScheduler::new(),
            curr_pid: 0,
            curr_sid: 0,
            running: 0,
            phantom: PhantomData,
        }
    }

    pub fn get_semaphore(&mut self, sid: Sid) -> Result<&Semaphore, ProcessError> {
        match self.stable.get_inner_spinlock_mut().get_mut().get(&sid) {
            Some(sema) => Ok(sema),
            None => Err(ProcessError::SemaphoreNotFound(sid)),
        }
    }

    pub fn get_semaphore_mut(&mut self, sid: Sid) -> Result<&mut Semaphore, ProcessError> {
        match self.stable.get_inner_spinlock_mut().get_mut().get_mut(&sid) {
            Some(sema) => Ok(sema),
            None => Err(ProcessError::SemaphoreNotFound(sid)),
        }
    }

    pub fn ptable_lock(&mut self) -> Lock<BTreeMap<Pid, Process>> {
        self.ptable.lock()
    }

    pub fn ptable_lock_mut(&mut self) -> LockMut<BTreeMap<Pid, Process>> {
        self.ptable.lock_mut()
    }

    pub fn load_program(&mut self, pid: Pid, path: &str) -> Result<(), ProcessError> {
        get_process_mut!(self.ptable_lock_mut(), pid)?
            .arch_proc
            .init_program(path);

        Ok(())
    }

    pub fn init(&mut self) -> Result<(), ProcessError> {
        let pid = self.create_process("null", 0, true)?;

        {
            let mut ptable = self.ptable_lock_mut();
            let proc = get_process_mut!(ptable, pid)?;
            proc.state = State::Running;
        }
        self.running = pid;

        Ok(())
    }

    pub fn create_semaphore(&mut self, count: isize) -> usize {
        let mask = interrupt_disable();

        let sid = self.curr_sid;
        self.curr_sid += 1;

        self.stable.lock_mut().insert(sid, Semaphore::new(count));

        interrupt_restore(mask);

        sid
    }

    pub fn wait_semaphore(&mut self, sid: Sid) -> Result<(), ProcessError> {
        let mask = interrupt_disable();

        let pid = self.running;
        self.get_semaphore_mut(sid)?.count -= 1;
        let count = self.get_semaphore_mut(sid)?.count;
        if count < 0 {
            get_process_mut!(self.ptable_lock_mut(), pid)?.state = State::SemaWait;
            self.get_semaphore_mut(sid)?.queue.push_back(pid);
            self.schedule()?;
        }

        interrupt_restore(mask);

        Ok(())
    }

    pub fn signal_semaphore(&mut self, sid: Sid) -> Result<(), ProcessError> {
        let mask = interrupt_disable();

        if self.get_semaphore(sid)?.count < 0 {
            let pid = self.get_semaphore_mut(sid)?.queue.pop_front().unwrap();
            self.get_semaphore_mut(sid)?.count += 1;
            self.ready(pid)?;
        } else {
            self.get_semaphore_mut(sid)?.count += 1;
        }

        interrupt_restore(mask);

        Ok(())
    }

    pub fn delete_semaphore(&mut self, sid: usize) -> Result<(), ProcessError> {
        let mask = interrupt_disable();

        self.get_semaphore_mut(sid)?.state = SemaState::Free;

        self.defer_schedule(DeferCommand::Start)?;

        while self.get_semaphore(sid)?.count < 0 {
            let pid = self.get_semaphore_mut(sid)?.queue.pop_front().unwrap();
            self.ready(pid)?;
            self.get_semaphore_mut(sid)?.count += 1;
        }

        self.defer_schedule(DeferCommand::Stop)?;

        self.schedule()?;

        interrupt_restore(mask);

        Ok(())
    }

    pub fn pop_ready_proc(&mut self) -> Result<Option<ProcessDesc>, ProcessError> {
        let proc_desc = &mut self.pqueue.lock_mut().pop();

        while let Some(p) = proc_desc {
            if get_process!(self.ptable_lock(), p.pid)?.state == State::Ready {
                break;
            }

            *proc_desc = self.pqueue.lock_mut().pop();
        }

        Ok((*proc_desc).clone())
    }

    pub fn defer_schedule(&mut self, cmd: DeferCommand) -> Result<(), ProcessError> {
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
                    self.schedule()?;
                }
            }
        }

        Ok(())
    }

    pub fn schedule(&mut self) -> Result<(), ProcessError> {
        let mask = interrupt_disable();

        if self.defer.count > 0 {
            self.defer.attempt = true;
            interrupt_restore(mask);
            return Ok(());
        }

        let proc = self.pop_ready_proc()?; // self.pqueue.pop();
        let pdesc = if let Some(desc) = proc {
            desc
        } else {
            interrupt_restore(mask);
            return Ok(());
        };

        let running = self.running;

        let ptable = self.ptable_lock();

        let old_pid = running;
        let new_pid = pdesc.pid;
        let old_priority = get_process!(ptable, running)?.priority;
        let new_priority = get_process!(ptable, pdesc.pid)?.priority;

        let old_context = (&get_process!(ptable, old_pid)?.arch_proc.context) as *const Context;
        let new_context = (&get_process!(ptable, new_pid)?.arch_proc.context) as *const Context;
        drop(ptable);

        if old_priority <= new_priority {
            let state = get_process!(self.ptable_lock(), running)?.state;
            if state == State::Running {
                get_process_mut!(self.ptable_lock_mut(), running)?.state = State::Ready;
                self.pqueue
                    .lock_mut()
                    .push(ProcessDesc::new(old_priority, old_pid));
            }
        } else {
            let state = get_process!(self.ptable_lock(), running)?.state;
            if state == State::Running {
                self.pqueue
                    .lock_mut()
                    .push(ProcessDesc::new(new_priority, new_pid));
                interrupt_restore(mask);
                return Ok(());
            }
        }

        get_process_mut!(self.ptable_lock_mut(), new_pid)?.state = State::Running;
        self.running = new_pid;

        // println!("[hobo0xcc] switch: {} -> {}", old_pid, new_pid);

        unsafe {
            context_switch(old_context as usize, new_context as usize);
        }
        interrupt_restore(mask);

        Ok(())
    }

    pub fn ready(&mut self, pid: Pid) -> Result<(), ProcessError> {
        let mut ptable = self.ptable_lock_mut();
        if get_process!(ptable, pid)?.state == State::Free
            || get_process!(ptable, pid)?.state == State::Running
            || get_process!(ptable, pid)?.state == State::Ready
        {
            return Ok(());
        }
        get_process_mut!(ptable, pid)?.state = State::Ready;
        let priority = get_process!(ptable, pid)?.priority;
        drop(ptable);

        self.pqueue.lock_mut().push(ProcessDesc::new(priority, pid));
        self.schedule()?;

        Ok(())
    }

    pub fn setup_kernel_process(&mut self, pid: Pid, func: usize) -> Result<(), ProcessError> {
        let mut ptable = self.ptable_lock_mut();
        let kernel_stack = get_process!(ptable, pid)?.kernel_stack;

        get_process_mut!(ptable, pid)?.arch_proc = ArchProcess::new(pid);

        get_process_mut!(ptable, pid)?
            .arch_proc
            .init(func, kernel_stack, KERNEL_STACK_SIZE);

        Ok(())
    }

    pub fn setup_process(&mut self, pid: usize) -> Result<(), ProcessError> {
        let mut ptable = self.ptable_lock_mut();
        let kernel_stack = get_process!(ptable, pid)?.kernel_stack;

        get_process_mut!(ptable, pid)?.arch_proc = ArchProcess::new(pid);

        get_process_mut!(ptable, pid)?.arch_proc.init(
            ArchProcess::user_trap_return as usize,
            kernel_stack,
            KERNEL_STACK_SIZE,
        );

        Ok(())
    }

    pub fn create_kernel_process(
        &mut self,
        name: &str,
        priority: usize,
        func: usize,
    ) -> Result<usize, ProcessError> {
        let pid = self.create_process(name, priority, false)?;
        self.setup_kernel_process(pid, func)?;

        Ok(pid)
    }

    pub fn create_process(
        &mut self,
        name: &str,
        priority: usize,
        do_setup: bool,
    ) -> Result<usize, ProcessError> {
        self.defer_schedule(DeferCommand::Start)?;
        let pid = self.curr_pid;

        self.curr_pid += 1;

        let mut ptable = self.ptable_lock_mut();

        ptable.insert(pid, Process::new(pid));

        let mut proc = get_process_mut!(ptable, pid)?;
        proc.priority = priority;

        proc.name = name.to_string();

        let stack_layout = Layout::from_size_align(KERNEL_STACK_SIZE, 0x1000).unwrap();
        let kernel_stack = unsafe { alloc(stack_layout) };
        proc.kernel_stack = kernel_stack as usize;

        // once a process created, the state of the process is setting up to State::Suspend
        // after create_process, the process need to be readied by `ready()`
        proc.state = State::Suspend;

        drop(proc);
        drop(ptable);

        if do_setup {
            self.setup_process(pid)?;
        }

        self.defer_schedule(DeferCommand::Stop)?;

        Ok(pid)
    }

    pub fn wakeup(&mut self) -> Result<(), ProcessError> {
        let mask = interrupt_disable();

        let ptr = self.sleep_queue.lock_mut().pop_front();
        if let Some(mut p) = ptr {
            let proc_delay = p.as_mut();
            if proc_delay.delay >= 1 {
                (*proc_delay).delay -= 1;
            }

            self.sleep_queue.lock_mut().push_front(p);

            let mut cursor = self
                .sleep_queue
                .get_inner_spinlock_mut()
                .get_mut()
                .front_mut();
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

            self.defer_schedule(DeferCommand::Start)?;
            for pid in pids.into_iter() {
                if get_process!(self.ptable_lock(), pid)?.state == State::Sleep {
                    self.ready(pid)?;
                }
            }
            self.defer_schedule(DeferCommand::Stop)?;

            self.schedule()?;
        }

        interrupt_restore(mask);

        Ok(())
    }

    pub fn sleep(&mut self, pid: Pid, delay: usize) -> Result<(), ProcessError> {
        let mask = interrupt_disable();
        get_process_mut!(self.ptable_lock_mut(), pid)?.state = State::Sleep;

        let mut que_spin = self.sleep_queue.lock_mut();
        let mut insert_node = que_spin.front_mut();
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

        drop(que_spin);

        self.schedule()?;

        interrupt_restore(mask);

        Ok(())
    }

    pub fn kill(&mut self, pid: Pid) -> Result<(), ProcessError> {
        let mask = interrupt_disable();

        let state = get_process!(self.ptable_lock(), pid)?.state;
        match state {
            State::Free => {}
            _ => {
                get_process_mut!(self.ptable_lock_mut(), pid)?.state = State::Free;
            }
        }

        get_process_mut!(self.ptable_lock_mut(), pid)?
            .arch_proc
            .free();
        let layout = Layout::from_size_align(KERNEL_STACK_SIZE, 0x1000).unwrap();
        unsafe {
            dealloc(
                get_process!(self.ptable_lock(), pid)?.kernel_stack as *mut u8,
                layout,
            );
        }

        self.event_signal(ProcessEvent::Exit(pid))?;

        self.schedule()?;

        interrupt_restore(mask);

        Ok(())
    }

    pub fn io_wait(&mut self, pid: Pid) -> Result<(), ProcessError> {
        let mask = interrupt_disable();

        get_process_mut!(self.ptable_lock_mut(), pid)?.state = State::IOWait;

        interrupt_restore(mask);

        Ok(())
    }

    pub fn io_signal(&mut self, pid: Pid) -> Result<(), ProcessError> {
        let mask = interrupt_disable();

        self.ready(pid)?;

        interrupt_restore(mask);

        Ok(())
    }

    // waiting for an `event` occurs
    // this is, for example, used to wait system call to wait for exiting of child process
    pub fn event_wait(&mut self, pid: Pid, event: ProcessEvent) -> Result<(), ProcessError> {
        let mask = interrupt_disable();

        get_process_mut!(self.ptable_lock_mut(), pid)?.state = State::EventWait;
        self.event_queue
            .entry(event)
            .or_insert_with(|| vec![])
            .push(pid);

        // scheduling is required because the `pid` might be a running process
        self.schedule()?;

        interrupt_restore(mask);

        Ok(())
    }

    // signaling a process waiting `event` to wakeup
    pub fn event_signal(&mut self, event: ProcessEvent) -> Result<(), ProcessError> {
        let mask = interrupt_disable();

        // deferring is required because the number of waiting processes might be greater than one
        self.defer_schedule(DeferCommand::Start)?;
        let events = self.event_queue.remove(&event);
        let events = if let Some(events) = events {
            events
        } else {
            // there are no waiting processes for the event
            self.defer_schedule(DeferCommand::Stop)?;
            interrupt_restore(mask);
            return Ok(());
        };
        for pid in events.iter() {
            self.ready(*pid)?;
        }
        self.defer_schedule(DeferCommand::Stop)?;

        self.schedule()?;

        interrupt_restore(mask);

        Ok(())
    }

    // wait for child processes to exit
    pub fn wait_exit(&mut self) -> Result<(), ProcessError> {
        let mask = interrupt_disable();

        let running = self.running;
        let mut ptable = self.ptable_lock_mut();
        for pid in get_process!(ptable, running)?.children.clone() {
            if get_process!(ptable, pid)?.state == State::Free {
                get_process_mut!(ptable, running)?.children.remove(pid);
                return Ok(());
            }
        }

        drop(ptable);

        self.defer_schedule(DeferCommand::Start)?;

        let running = self.running;
        let children = get_process!(self.ptable_lock(), running)?.children.clone();
        for pid in children {
            self.event_wait(self.running, ProcessEvent::Exit(pid))?;
        }
        self.defer_schedule(DeferCommand::Stop)?;

        self.schedule()?;

        interrupt_restore(mask);

        Ok(())
    }

    pub fn fork(&mut self, _pid: Pid) -> usize {
        0
    }

    pub fn get_process_state(&mut self, pid: Pid) -> Result<State, ProcessError> {
        let ptable = self.ptable_lock();
        let proc = get_process!(ptable, pid)?;
        Ok(proc.state)
    }
}

pub unsafe fn process_manager() -> &'static mut ProcessManager<'static> {
    match PM {
        Some(ref mut pm) => &mut *pm,
        None => panic!("process manager is uninitialized"),
    }
}

pub fn init() {
    let pm = ProcessManager::new();
    unsafe {
        PM = Some(pm);
        let pm = process_manager();
        pm.init().expect("process");
    }
}
