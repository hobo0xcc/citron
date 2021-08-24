pub mod virtio_blk;
pub mod virtio_gpu;
pub mod virtio_input;

use super::{layout, plic};
use crate::arch::riscv64::virtio::virtio_input::DeviceType;
use crate::*;

pub static mut BLOCK_DEVICE: Option<virtio_blk::VirtioBlk> = None;
pub static mut GPU_DEVICE: Option<virtio_gpu::VirtioGpu> = None;
pub static mut MOUSE_DEVICE: Option<virtio_input::VirtioInput> = None;
pub static mut KEYBOARD_DEVICE: Option<virtio_input::VirtioInput> = None;

#[derive(Copy, Clone)]
pub enum VirtioReg {
    MagicValue = 0x000,
    Version = 0x004,
    DeviceId = 0x008,
    VendorId = 0x00c,
    DeviceFeatures = 0x010,
    DeviceFeaturesSel = 0x014,
    DriverFeatures = 0x020,
    DriverFeaturesSel = 0x024,
    QueueSel = 0x030,
    QueueNumMax = 0x034,
    QueueNum = 0x038,
    QueueReady = 0x044,
    QueueNotify = 0x050,
    InterruptStatus = 0x060,
    InterruptACK = 0x064,
    Status = 0x070,
    QueueDescLow = 0x080,
    QueueDescHigh = 0x084,
    QueueDriverLow = 0x090,
    QueueDriverHigh = 0x094,
    QueueDeviceLow = 0x0a0,
    QueueDeviceHigh = 0x0a4,
    ConfigGeneration = 0x0fc,
    Config = 0x100,
}

impl VirtioReg {
    pub fn val(&self) -> usize {
        *self as usize
    }
}

#[derive(Copy, Clone)]
pub enum VirtioDeviceStatus {
    Acknowoledge = 1,
    Driver = 2,
    Failed = 128,
    FeaturesOk = 8,
    DriverOk = 4,
    DeviceNeedsReset = 64,
}

impl VirtioDeviceStatus {
    pub fn val(&self) -> u32 {
        *self as u32
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct Virtqueue {
    desc: *mut VirtqDesc,
    avail: *mut VirtqAvail,
    used: *mut VirtqUsed,
}

impl Virtqueue {
    pub fn new(desc: *mut VirtqDesc, avail: *mut VirtqAvail, used: *mut VirtqUsed) -> Self {
        Virtqueue { desc, avail, used }
    }
}

pub const VIRTIO_RING_SIZE: usize = 1 << 7;
#[derive(Copy, Clone)]
pub enum VirtqDescFlag {
    VirtqDescFNext = 1,
    VirtqDescFWrite = 2,
    VirtqDescFIndirect = 4,
}

impl VirtqDescFlag {
    pub fn val(&self) -> u16 {
        *self as u16
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

impl VirtqDesc {
    pub fn new(addr: u64, len: u32, flags: u16, next: u16) -> Self {
        VirtqDesc {
            addr,
            len,
            flags,
            next,
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VirtqAvail {
    flags: u16,
    idx: u16,
    ring: [u16; VIRTIO_RING_SIZE],
    used_event: u16,
}

impl VirtqAvail {
    pub fn new(flags: u16, idx: u16, used_event: u16) -> Self {
        VirtqAvail {
            flags,
            idx,
            ring: [0; VIRTIO_RING_SIZE],
            used_event,
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VirtqUsedElem {
    id: u32,
    len: u32,
}

impl VirtqUsedElem {
    pub fn new(id: u32, len: u32) -> Self {
        VirtqUsedElem { id, len }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VirtqUsed {
    flags: u16,
    idx: u16,
    ring: [VirtqUsedElem; VIRTIO_RING_SIZE],
    avail_event: u16,
}

impl VirtqUsed {
    pub fn new(flags: u16, idx: u16, avail_event: u16) -> Self {
        VirtqUsed {
            flags,
            idx,
            ring: [VirtqUsedElem::new(0, 0); VIRTIO_RING_SIZE],
            avail_event,
        }
    }
}

pub fn read_reg32(base: usize, offset: usize) -> u32 {
    let ptr = (base + offset) as *mut u32;
    unsafe { ptr.read_volatile() }
}

pub fn read_reg64(base: usize, offset: usize) -> u64 {
    let ptr = (base + offset) as *mut u64;
    unsafe { ptr.read_volatile() }
}

pub fn write_reg32(base: usize, offset: usize, val: u32) {
    let ptr = (base + offset) as *mut u32;
    unsafe {
        ptr.write_volatile(val);
    }
}

pub fn write_reg64(base: usize, offset: usize, val: u64) {
    let ptr = (base + offset) as *mut u64;
    unsafe {
        ptr.write_volatile(val);
    }
}

pub unsafe fn block_device() -> &'static mut virtio_blk::VirtioBlk {
    match BLOCK_DEVICE {
        Some(ref mut blk) => blk,
        None => panic!("block device is uninitialized"),
    }
}

pub unsafe fn gpu_device() -> &'static mut virtio_gpu::VirtioGpu {
    match GPU_DEVICE {
        Some(ref mut gpu) => gpu,
        None => panic!("gpu device is uninitialized"),
    }
}

pub unsafe fn mouse_device() -> &'static mut virtio_input::VirtioInput {
    match MOUSE_DEVICE {
        Some(ref mut mouse) => mouse,
        None => panic!("mouse device is uninitialized"),
    }
}

pub unsafe fn keyboard_device() -> &'static mut virtio_input::VirtioInput {
    match KEYBOARD_DEVICE {
        Some(ref mut keyboard) => keyboard,
        None => panic!("keyboard device is uninitialized"),
    }
}

pub fn interrupt(irq: u32) {
    let index = irq as usize - plic::Irq::VirtioFirstIrq.val();
    match index {
        0 => {
            let blk = unsafe { block_device() };
            blk.pending();
        }
        1 => {
            let gpu = unsafe { gpu_device() };
            gpu.pending();
        }
        2 => {
            let mouse = unsafe { mouse_device() };
            mouse.pending();
        }
        3 => {
            let keyboard = unsafe { keyboard_device() };
            keyboard.pending();
        }
        _ => panic!("unknown virtio device: {}", index),
    }
}

#[allow(unaligned_references)]
pub fn init() {
    let virtio_base = layout::_virtio_start as usize;
    // let virtio_end = layout::_virtio_end as usize;

    for i in 0..4 {
        let offset = i * 0x1000;
        let ptr = virtio_base + offset;
        if read_reg32(ptr, VirtioReg::MagicValue.val()) != 0x74726976 {
            continue;
        }

        match read_reg32(ptr, VirtioReg::DeviceId.val()) {
            2 => {
                let blk = virtio_blk::init(ptr);
                println!("virtio_blk: {:#018x}", ptr);
                unsafe {
                    BLOCK_DEVICE = Some(blk);
                }
            }
            16 => {
                let gpu = virtio_gpu::init(ptr);
                println!("virtio_gpu: {:#018x}", ptr);
                unsafe {
                    GPU_DEVICE = Some(gpu);
                }
            }
            18 => {
                let device_type = match i {
                    2 => DeviceType::Mouse,
                    3 => DeviceType::Keyboard,
                    _ => unimplemented!(),
                };
                let input = virtio_input::init(ptr, device_type);
                println!("virtio_input: {:#018x}", ptr);
                unsafe {
                    match i {
                        2 => {
                            MOUSE_DEVICE = Some(input);
                        }
                        3 => {
                            KEYBOARD_DEVICE = Some(input);
                        }
                        _ => {}
                    }
                }
            }
            _ => {
                println!(
                    "unknown virtio-device: {}",
                    read_reg32(ptr, VirtioReg::DeviceId.val())
                );
            }
        }
    }
}
