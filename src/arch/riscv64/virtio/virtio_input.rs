use super::super::virtio;
use super::*;
use crate::arch::riscv64::interrupt::*;
use crate::process::process_manager;
use alloc::alloc::{alloc, alloc_zeroed, Layout};
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::mem::size_of;
use core::ptr::NonNull;
use volatile_register::*;

pub const EVENT_BUFFER_SIZE: usize = VIRTIO_RING_SIZE;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DeviceType {
    Mouse,
    Keyboard,
}

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum VirtioInputConfigSelect {
    InputCfgUnset = 0x00,
    InputCfgIdName = 0x01,
    InputCfgIdSerial = 0x02,
    InputCfgIdDevids = 0x03,
    InputCfgPropBits = 0x10,
    InputCfgEvBits = 0x11,
    InputCfgAbsInfo = 0x12,
}

impl VirtioInputConfigSelect {
    pub fn val(&self) -> u8 {
        *self as u8
    }
}

// from linux kernel: include/linux/input.h
#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
#[repr(u16)]
pub enum EventType {
    EV_SYN = 0,
    EV_KEY = 1,
    EV_REL = 2,
    EV_ABS = 3,
    EV_MSC = 4,
    EV_SW = 5,
    EV_LED = 17,
    EV_SND = 18,
    EV_REP = 20,
    EV_FF = 21,
    EV_PWR = 22,
    EV_FF_STATUS = 23,
    EV_UNK,
    EV_MAX = 31,
}

impl EventType {
    pub fn from(type_: u16) -> EventType {
        match type_ {
            0 => EventType::EV_SYN,
            1 => EventType::EV_KEY,
            2 => EventType::EV_REL,
            3 => EventType::EV_ABS,
            4 => EventType::EV_MSC,
            5 => EventType::EV_SW,
            17 => EventType::EV_LED,
            18 => EventType::EV_SND,
            20 => EventType::EV_REP,
            21 => EventType::EV_FF,
            22 => EventType::EV_PWR,
            23 => EventType::EV_FF_STATUS,
            31 => EventType::EV_MAX,
            _ => panic!("unknown event type: {}", type_),
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
#[repr(u8)]
pub enum EV_REL {
    REL_X = 0,
    REL_Y = 1,
    REL_Z = 2,
    REL_RX = 3,
    REL_RY = 4,
    REL_RZ = 5,
    REL_HWHEEL = 6,
    REL_DIAL = 7,
    REL_WHEEL = 8,
    REL_MISC = 9,
    REL_RESERVED = 10,
    REL_WHEEL_HI_RES = 11,
    REL_HWHEEL_HI_RES = 12,
    REL_MAX = 15,
}

#[repr(C, packed)]
pub struct VirtioInputAbsInfo {
    min: RW<u32>,
    max: RW<u32>,
    fuzz: RW<u32>,
    flat: RW<u32>,
    res: RW<u32>,
}

#[repr(C, packed)]
pub struct VirtioInputDevids {
    bustype: RW<u16>,
    vendor: RW<u16>,
    product: RW<u16>,
    version: RW<u16>,
}

#[repr(C, packed)]
pub struct VirtioInputConfig {
    select: RW<u8>,
    subsel: RW<u8>,
    size: RW<u8>,
    reserved: [RW<u8>; 5],
    // u: [RW<u8>; 128],
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct VirtioInputEvent {
    pub type_: u16,
    pub code: u16,
    pub value: u32,
}

impl Default for VirtioInputEvent {
    fn default() -> Self {
        VirtioInputEvent {
            type_: 0,
            code: 0,
            value: 0,
        }
    }
}

#[derive(Copy, Clone)]
pub enum VirtioInputQueue {
    Eventq = 0,
    Statusq = 1,
}

#[allow(dead_code)]
pub struct VirtioInput {
    base: usize,
    device_type: DeviceType,
    virtqueue: [NonNull<Virtqueue>; 2],
    event_buffer: *mut VirtioInputEvent,
    curr_queue: VirtioInputQueue,
    free_desc: [bool; VIRTIO_RING_SIZE], // true if the desc is free
    desc_indexes: Option<Vec<u16>>,
    event_ack_used_index: u16,
    event_index: u16,
    status_ack_used_index: u16,
    pub event_queue: VecDeque<VirtioInputEvent>,
    sid: usize,
    pid: usize,
}

impl VirtioInput {
    pub fn new(base: usize, device_type: DeviceType) -> Self {
        let pm = unsafe { process_manager() };
        let layout =
            Layout::from_size_align(size_of::<VirtioInputEvent>() * EVENT_BUFFER_SIZE, 8).unwrap();
        let event_buffer = unsafe { alloc_zeroed(layout) } as *mut VirtioInputEvent;
        VirtioInput {
            base,
            device_type,
            virtqueue: [NonNull::dangling(); 2],
            event_buffer,
            curr_queue: VirtioInputQueue::Eventq,
            free_desc: [true; VIRTIO_RING_SIZE],
            desc_indexes: None,
            event_ack_used_index: 0,
            event_index: 0,
            status_ack_used_index: 0,
            event_queue: VecDeque::new(),
            sid: pm.create_semaphore(1),
            pid: 0,
        }
    }

    pub fn read_reg32(&mut self, offset: usize) -> u32 {
        virtio::read_reg32(self.base, offset)
    }

    pub fn read_reg64(&mut self, offset: usize) -> u64 {
        virtio::read_reg64(self.base, offset)
    }

    pub fn write_reg32(&mut self, offset: usize, val: u32) {
        virtio::write_reg32(self.base, offset, val)
    }

    pub fn write_reg64(&mut self, offset: usize, val: u64) {
        virtio::write_reg64(self.base, offset, val)
    }

    pub fn init_virtq(&mut self, queue: VirtioInputQueue) {
        assert_eq!(size_of::<VirtqDesc>(), 16);
        let desc_layout = Layout::from_size_align(16 * VIRTIO_RING_SIZE, 16).unwrap();
        let desc = unsafe { alloc_zeroed(desc_layout) } as *mut VirtqDesc;

        assert_eq!(size_of::<VirtqAvail>(), 6 + 2 * VIRTIO_RING_SIZE);
        let avail_layout = Layout::from_size_align(6 + 2 * VIRTIO_RING_SIZE, 2).unwrap();
        let avail = unsafe { alloc_zeroed(avail_layout) } as *mut VirtqAvail;

        assert_eq!(size_of::<VirtqUsed>(), 6 + 8 * VIRTIO_RING_SIZE);
        let used_layout = Layout::from_size_align(6 + 8 * VIRTIO_RING_SIZE, 2).unwrap();
        let used = unsafe { alloc_zeroed(used_layout) } as *mut VirtqUsed;

        assert_eq!(size_of::<Virtqueue>(), 24);
        let virtqueue_layout = Layout::from_size_align(size_of::<Virtqueue>(), 8).unwrap();
        let virtqueue = unsafe { alloc(virtqueue_layout) } as *mut Virtqueue;
        unsafe {
            *virtqueue = Virtqueue::new(desc, avail, used);
        }

        self.virtqueue[queue as usize] = NonNull::new(virtqueue).unwrap();
    }

    pub fn find_free_desc(&mut self) -> u16 {
        for (i, is_free) in self.free_desc.iter_mut().enumerate() {
            if *is_free {
                *is_free = false;
                return i as u16;
            }
        }

        panic!("free desc exhausted");
    }

    pub fn allocate_desc(&mut self, n: usize, indexes: &mut Vec<u16>) {
        for _ in 0..n {
            let index = self.find_free_desc();
            indexes.push(index);
        }
    }

    pub fn deallocate_desc(&mut self, indexes: &Vec<u16>) {
        for i in indexes.iter() {
            self.free_desc[*i as usize] = true;
        }
    }

    pub fn write_desc(&mut self, i: usize, queue: VirtioInputQueue, desc: VirtqDesc) {
        unsafe {
            let desc_ptr = self.virtqueue[queue as usize].as_mut().desc.add(i);
            *desc_ptr = desc;
        }
    }

    pub fn send_desc(&mut self, queue: VirtioInputQueue, desc_indexes: Vec<u16>) {
        unsafe {
            let virtqueue = self.virtqueue[queue as usize].as_mut();
            self.write_reg64(VirtioReg::QueueDescLow.val(), virtqueue.desc as u64);
            self.write_reg64(VirtioReg::QueueDriverLow.val(), virtqueue.avail as u64);
            self.write_reg64(VirtioReg::QueueDeviceLow.val(), virtqueue.used as u64);
            self.curr_queue = queue;

            let mut avail = virtqueue.avail.as_mut().unwrap();
            let index = avail.idx as usize;
            avail.ring[index % VIRTIO_RING_SIZE] = desc_indexes[0];
            asm!("fence iorw, iorw");
            avail.idx = avail.idx.wrapping_add(1);
            asm!("fence iorw, iorw");
            // self.desc_indexes = Some(desc_indexes);
            // pm.io_wait(self.pid);
            // self.write_reg32(VirtioReg::QueueNotify.val(), queue as u32);
            // pm.schedule();
        }
    }

    pub fn init(&mut self) {
        let pm = unsafe { process_manager() };

        pm.wait_semaphore(self.sid);

        let magic_value = self.read_reg32(VirtioReg::MagicValue.val());
        let version = self.read_reg32(VirtioReg::Version.val());
        let device_id = self.read_reg32(VirtioReg::DeviceId.val());
        if magic_value != 0x74726976 || version != 2 || device_id != 18 {
            panic!("unrecognized virtio device: {:#018x}", self.base);
        }

        let mut status_bits: u32 = 0;
        status_bits |= VirtioDeviceStatus::Acknowoledge.val();
        self.write_reg32(VirtioReg::Status.val(), status_bits);

        status_bits |= VirtioDeviceStatus::Driver.val();
        self.write_reg32(VirtioReg::Status.val(), status_bits);

        let features = self.read_reg32(VirtioReg::DeviceFeatures.val());
        self.write_reg32(VirtioReg::DeviceFeatures.val(), features);

        status_bits |= VirtioDeviceStatus::FeaturesOk.val();
        self.write_reg32(VirtioReg::Status.val(), status_bits);

        if self.read_reg32(VirtioReg::Status.val()) & VirtioDeviceStatus::FeaturesOk.val() == 0 {
            self.write_reg32(VirtioReg::Status.val(), VirtioDeviceStatus::Failed.val());
            panic!(
                "virtio-blk({:#018x}) does not support the required features",
                self.base
            );
        }

        self.write_reg32(VirtioReg::QueueSel.val(), 0);

        if self.read_reg32(VirtioReg::QueueReady.val()) != 0 {
            panic!("queue is already in use");
        }

        let queue_num_max = self.read_reg32(VirtioReg::QueueNumMax.val());
        if queue_num_max == 0 {
            panic!("queue is not available");
        } else if queue_num_max < (VIRTIO_RING_SIZE as u32) {
            panic!("QueueNumMax too short");
        }

        self.write_reg32(VirtioReg::QueueNum.val(), VIRTIO_RING_SIZE as u32);

        self.init_virtq(VirtioInputQueue::Eventq);
        self.init_virtq(VirtioInputQueue::Statusq);

        self.write_reg32(VirtioReg::QueueNum.val(), VIRTIO_RING_SIZE as u32);

        let virtqueue = unsafe { self.virtqueue[0].as_mut() };
        self.write_reg64(VirtioReg::QueueDescLow.val(), virtqueue.desc as u64);
        self.write_reg64(VirtioReg::QueueDriverLow.val(), virtqueue.avail as u64);
        self.write_reg64(VirtioReg::QueueDeviceLow.val(), virtqueue.used as u64);

        self.write_reg32(VirtioReg::QueueReady.val(), 1);

        status_bits |= VirtioDeviceStatus::DriverOk.val();
        self.write_reg32(VirtioReg::Status.val(), status_bits);

        pm.signal_semaphore(self.sid);
    }

    pub fn setup_config(&mut self) {
        let pm = unsafe { process_manager() };
        pm.wait_semaphore(self.sid);

        let config = (self.base + VirtioReg::Config as usize) as *mut VirtioInputConfig;
        unsafe {
            pm.io_wait(self.pid);
            (*config)
                .select
                .write(VirtioInputConfigSelect::InputCfgIdName.val());
            pm.schedule();

            let mut name = String::new();
            let size = (*config).size.read();
            let u = (self.base + VirtioReg::Config as usize + 8) as *mut u8;
            for i in 0..size as usize {
                let ch = u.add(i).read_volatile() as char;
                name.push(ch);
            }
            println!("input device detected: {}", name);

            let event_type = match self.device_type {
                DeviceType::Mouse => EventType::EV_REL,
                DeviceType::Keyboard => EventType::EV_KEY,
            };

            pm.io_wait(self.pid);
            (*config).subsel.write(event_type as u8);
            (*config)
                .select
                .write(VirtioInputConfigSelect::InputCfgEvBits as u8);
            pm.schedule();
        }

        pm.signal_semaphore(self.sid);
    }

    pub fn repopulate_event(&mut self, i: usize) {
        let buffer = unsafe { self.event_buffer.add(i) };
        let flag = VirtqDescFlag::VirtqDescFWrite.val();
        let desc = VirtqDesc::new(buffer as u64, size_of::<VirtioInputEvent>() as u32, flag, 0);

        let head = self.event_index;
        self.write_desc(self.event_index as usize, VirtioInputQueue::Eventq, desc);
        self.event_index = (self.event_index + 1) % VIRTIO_RING_SIZE as u16;

        let desc_indexes = vec![head];
        self.send_desc(VirtioInputQueue::Eventq, desc_indexes);
    }

    pub fn init_input_event(&mut self) {
        self.setup_config();

        let pm = unsafe { process_manager() };
        pm.wait_semaphore(self.sid);

        self.pid = pm.running;

        for i in 0..(EVENT_BUFFER_SIZE / 2) {
            self.repopulate_event(i);
        }

        // self.send_desc(VirtioInputQueue::Eventq, desc_indexes);
        pm.signal_semaphore(self.sid);
    }

    pub fn pending(&mut self) {
        // println!("virtio_input pending start");
        let mask = interrupt_disable();

        let interrupt_status = self.read_reg32(VirtioReg::InterruptStatus.val());
        self.write_reg32(VirtioReg::InterruptACK.val(), interrupt_status & 0x3);
        let virtqueue = unsafe { self.virtqueue[self.curr_queue as usize].as_mut() };
        let desc = virtqueue.desc;
        let used = unsafe { virtqueue.used.as_mut().unwrap() };

        while self.event_ack_used_index != used.idx {
            let index = self.event_ack_used_index % VIRTIO_RING_SIZE as u16;
            let elem = used.ring[index as usize];

            self.repopulate_event(elem.id as usize);

            self.event_ack_used_index = self.event_ack_used_index.wrapping_add(1);
            unsafe {
                let desc = desc.add(elem.id as usize).as_mut().unwrap();
                let event = (desc.addr as *mut VirtioInputEvent).as_mut().unwrap();
                self.event_queue.push_back(*event);
                // println!("{:?}", event);
            }
        }

        let pm = unsafe { process_manager() };
        pm.io_signal(self.pid);

        interrupt_restore(mask);
        // println!("virtio_input pending end");
    }
}

pub fn init(base: usize, device_type: DeviceType) -> VirtioInput {
    let mut input = VirtioInput::new(base, device_type);
    input.init();
    input
}
