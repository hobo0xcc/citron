use crate::arch::riscv64::interrupt::interrupt_disable;
use crate::arch::riscv64::interrupt::interrupt_restore;
use crate::arch::riscv64::interrupt::is_interrupt_enable;
use crate::process::process_manager;
use alloc::alloc::{alloc, alloc_zeroed, dealloc};
use alloc::vec::Vec;
use core::alloc::Layout;
use core::mem::size_of;
use core::ptr::{null_mut, NonNull};

use super::super::virtio;
use super::*;

#[derive(Copy, Clone)]
pub enum VirtioGpuFeature {
    VirtioGpuFVirgl = 0,
    VirtioGpuFEdid = 1,
}

impl VirtioGpuFeature {
    pub fn val(&self) -> u32 {
        *self as u32
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct VirtioGpuCtrlHdr {
    pub type_: u32,
    pub flags: u32,
    pub fence_id: u64,
    pub ctx_id: u32,
    pub padding: u32,
}

#[derive(Copy, Clone, Debug)]
pub enum VirtioGpuCtrlType {
    /* 2d commands */
    CmdGetDisplayInfo = 0x0100,
    CmdResourceCreate2d,
    CmdResourceUnref,
    CmdSetScanout,
    CmdResourceFlush,
    CmdTransferToHost2d,
    CmdResourceAttachBacking,
    CmdResourceDetachBacking,
    CmdGetCapsetInfo,
    CmdGetCapset,
    CmdGetEdid,
    /* cursor commands */
    CmdUpdateCursor = 0x0300,
    CmdMoveCursor,
    /* success responses */
    RespOkNodata = 0x1100,
    RespOkDisplayInfo,
    RespOkCapsetInfo,
    RespOkCapset,
    RespOkEdid,
    /* error responses */
    RespErrUnspec = 0x1200,
    RespErrOutOfMemory,
    RespErrInvalidScanoutId,
    RespErrInvalidResourceId,
    RespErrInvalidContextId,
    RespErrInvalidParameter,
}

impl VirtioGpuCtrlType {
    pub fn val(&self) -> u32 {
        *self as u32
    }
}

const VIRTIO_GPU_MAX_SCANOUTS: usize = 16;

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct VirtioGpuRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct VirtioGpuDisplayOne {
    pub r: VirtioGpuRect,
    pub enabled: u32,
    pub flags: u32,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct VirtioGpuRespDisplayInfo {
    pub hdr: VirtioGpuCtrlHdr,
    pub pmodes: [VirtioGpuDisplayOne; VIRTIO_GPU_MAX_SCANOUTS],
}

#[derive(Copy, Clone)]
pub enum VirtioGpuFormats {
    B8G8R8A8Unorm = 1,
    B8G8R8X8Unorm = 2,
    A8R8G8B8Unorm = 3,
    X8R8G8B8Unorm = 4,
    R8G8B8A8Unorm = 67,
    X8B8G8R8Unorm = 68,
    A8B8G8R8Unorm = 121,
    R8G8B8X8Unorm = 134,
}

impl VirtioGpuFormats {
    pub fn val(&self) -> u32 {
        *self as u32
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct VirtioGpuResourceCreate2d {
    hdr: VirtioGpuCtrlHdr,
    resource_id: u32,
    format: u32,
    width: u32,
    height: u32,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct VirtioGpuResourceAttachBacking {
    hdr: VirtioGpuCtrlHdr,
    resource_id: u32,
    nr_entries: u32,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct VirtioGpuMemEntry {
    addr: u64,
    length: u32,
    padding: u32,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct VirtioGpuSetScanout {
    hdr: VirtioGpuCtrlHdr,
    r: VirtioGpuRect,
    scanout_id: u32,
    resource_id: u32,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct VirtioGpuTransferToHost2d {
    hdr: VirtioGpuCtrlHdr,
    r: VirtioGpuRect,
    offset: u64,
    resource_id: u32,
    padding: u32,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct VirtioGpuResourceFlush {
    hdr: VirtioGpuCtrlHdr,
    r: VirtioGpuRect,
    resource_id: u32,
    padding: u32,
}

#[repr(C, packed)]
pub struct Request<RqT, RpT> {
    request: RqT,
    response: RpT,
}

impl<RqT, RpT> Request<RqT, RpT> {
    pub fn new(request: RqT) -> *mut Self {
        let size = size_of::<RqT>() + size_of::<RpT>();
        let layout = Layout::from_size_align(size, 8).unwrap();
        let ptr = unsafe { alloc(layout) } as *mut Self;
        unsafe {
            (*ptr).request = request;
        }
        ptr
    }
}

pub struct Request3<RqT, RmT, RpT> {
    request: RqT,
    mementries: RmT,
    response: RpT,
}

impl<RqT, RmT, RpT> Request3<RqT, RmT, RpT> {
    pub fn new(request: RqT, meminfo: RmT) -> *mut Self {
        let size = size_of::<RqT>() + size_of::<RmT>() + size_of::<RpT>();
        let layout = Layout::from_size_align(size, 8).unwrap();
        let ptr = unsafe { alloc(layout) } as *mut Self;
        unsafe {
            (*ptr).request = request;
            (*ptr).mementries = meminfo;
        }
        ptr
    }
}

#[allow(dead_code)]
const VIRTIO_GPU_FLAG_FENCE: usize = 1 << 0;
const PIXEL_SIZE: u32 = 4;

#[derive(Copy, Clone)]
pub enum VirtioGpuQueue {
    Controlq = 0,
    Cursorq = 1,
}

pub struct VirtioGpu {
    base: usize,
    pub framebuffer: *mut u8,
    virtqueue: [NonNull<Virtqueue>; 2],
    curr_queue: VirtioGpuQueue,
    free_desc: [bool; VIRTIO_RING_SIZE], // true if the desc is free
    desc_indexes: Option<Vec<u16>>,
    ack_used_index: u16,
    resource_id: u32,
    pub width: u32,
    pub height: u32,
    sid: usize, // Semaphore id
    pid: usize,
}

impl VirtioGpu {
    pub fn new(base: usize) -> Self {
        let pm = unsafe { process_manager() };
        VirtioGpu {
            base,
            framebuffer: null_mut(),
            virtqueue: [NonNull::dangling(); 2],
            curr_queue: VirtioGpuQueue::Controlq,
            free_desc: [true; VIRTIO_RING_SIZE],
            desc_indexes: None,
            ack_used_index: 0,
            resource_id: 1,
            width: 0,
            height: 0,
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

    pub fn init_virtq(&mut self, queue: VirtioGpuQueue) {
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

    pub fn init(&mut self) {
        let pm = unsafe { process_manager() };

        pm.wait_semaphore(self.sid);

        let magic_value = self.read_reg32(VirtioReg::MagicValue.val());
        let version = self.read_reg32(VirtioReg::Version.val());
        let device_id = self.read_reg32(VirtioReg::DeviceId.val());
        if magic_value != 0x74726976 || version != 2 || device_id != 16 {
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

        self.init_virtq(VirtioGpuQueue::Controlq);
        self.init_virtq(VirtioGpuQueue::Cursorq);

        // assert_eq!(size_of::<VirtqDesc>(), 16);
        // let desc_layout = Layout::from_size_align(16 * VIRTIO_RING_SIZE, 16).unwrap();
        // let desc = unsafe { alloc_zeroed(desc_layout) } as *mut VirtqDesc;

        // assert_eq!(size_of::<VirtqAvail>(), 6 + 2 * VIRTIO_RING_SIZE);
        // let avail_layout = Layout::from_size_align(6 + 2 * VIRTIO_RING_SIZE, 2).unwrap();
        // let avail = unsafe { alloc_zeroed(avail_layout) } as *mut VirtqAvail;

        // assert_eq!(size_of::<VirtqUsed>(), 6 + 8 * VIRTIO_RING_SIZE);
        // let used_layout = Layout::from_size_align(6 + 8 * VIRTIO_RING_SIZE, 2).unwrap();
        // let used = unsafe { alloc_zeroed(used_layout) } as *mut VirtqUsed;

        // assert_eq!(size_of::<Virtqueue>(), 24);
        // let virtqueue_layout = Layout::from_size_align(size_of::<Virtqueue>(), 8).unwrap();
        // let virtqueue = unsafe { alloc(virtqueue_layout) } as *mut Virtqueue;
        // unsafe {
        //     *virtqueue = Virtqueue::new(desc, avail, used);
        // }

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

    pub fn init_display(&mut self) {
        let pm = unsafe { process_manager() };
        pm.wait_semaphore(self.sid);
        self.pid = pm.running;
        // virtio_gpu settings
        let display_info = self.get_display_info();
        self.width = display_info.pmodes[0].r.width;
        self.height = display_info.pmodes[0].r.height;

        let resource_id =
            self.resource_create_2d(self.width, self.height, VirtioGpuFormats::R8G8B8A8Unorm);

        self.init_framebuffer(self.width, self.height, PIXEL_SIZE);

        self.resource_attach_backing(self.width, self.height, PIXEL_SIZE, resource_id);

        self.set_scanout(self.width, self.height, resource_id);

        pm.signal_semaphore(self.sid);
    }

    pub fn get_pixel_size(&mut self) -> u32 {
        PIXEL_SIZE
    }

    pub fn get_framebuffer(&mut self) -> *mut u8 {
        self.framebuffer
    }

    pub fn init_framebuffer(&mut self, width: u32, height: u32, pixel_size: u32) {
        let size = width * pixel_size * height;
        let layout = Layout::from_size_align(size as usize, 0x1000).unwrap();
        let framebuffer = unsafe { alloc_zeroed(layout) };
        self.framebuffer = framebuffer;
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

    pub fn write_desc(&mut self, i: usize, queue: VirtioGpuQueue, desc: VirtqDesc) {
        unsafe {
            let desc_ptr = self.virtqueue[queue as usize].as_mut().desc.add(i);
            *desc_ptr = desc;
        }
    }

    pub fn send_desc(&mut self, queue: VirtioGpuQueue, desc_indexes: Vec<u16>) {
        unsafe {
            let virtqueue = self.virtqueue[queue as usize].as_mut();
            self.write_reg64(VirtioReg::QueueDescLow.val(), virtqueue.desc as u64);
            self.write_reg64(VirtioReg::QueueDriverLow.val(), virtqueue.avail as u64);
            self.write_reg64(VirtioReg::QueueDeviceLow.val(), virtqueue.used as u64);
            self.curr_queue = queue;

            let pm = process_manager();

            let mut avail = virtqueue.avail.as_mut().unwrap();
            let index = avail.idx as usize;
            avail.ring[index % VIRTIO_RING_SIZE] = desc_indexes[0];
            asm!("fence iorw, iorw");
            avail.idx = avail.idx.wrapping_add(1);
            asm!("fence iorw, iorw");
            self.desc_indexes = Some(desc_indexes);
            pm.io_wait(self.pid);
            self.write_reg32(VirtioReg::QueueNotify.val(), queue as u32);
            if !is_interrupt_enable() {
                println!("interrupt isn't enabled!");
            }
            pm.schedule();
        }
    }

    pub fn update_range(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.transfer_to_host_2d(x, y, width, height, self.resource_id);
        self.resource_flush(x, y, width, height, self.resource_id);
    }

    pub fn update_display(&mut self) {
        self.transfer_to_host_2d(0, 0, self.width, self.height, self.resource_id);
        self.resource_flush(0, 0, self.width, self.height, self.resource_id);
    }

    pub fn get_display_info(&mut self) -> VirtioGpuRespDisplayInfo {
        let req = Request::<VirtioGpuCtrlHdr, VirtioGpuRespDisplayInfo>::new(VirtioGpuCtrlHdr {
            type_: VirtioGpuCtrlType::CmdGetDisplayInfo.val(),
            flags: 0,
            fence_id: 0,
            ctx_id: 0,
            padding: 0,
        });

        let mut desc_indexes = Vec::new();
        self.allocate_desc(2, &mut desc_indexes);

        // request desc
        let desc = VirtqDesc::new(
            unsafe { &(*req).request as *const VirtioGpuCtrlHdr as u64 },
            size_of::<VirtioGpuCtrlHdr>() as u32,
            VirtqDescFlag::VirtqDescFNext.val(),
            desc_indexes[1],
        );
        self.write_desc(desc_indexes[0] as usize, VirtioGpuQueue::Controlq, desc);

        // response desc
        let desc = VirtqDesc::new(
            unsafe { &(*req).response as *const VirtioGpuRespDisplayInfo as u64 },
            size_of::<VirtioGpuRespDisplayInfo>() as u32,
            VirtqDescFlag::VirtqDescFWrite.val(),
            0,
        );
        self.write_desc(desc_indexes[1] as usize, VirtioGpuQueue::Controlq, desc);

        // send an request and wait for response
        self.send_desc(VirtioGpuQueue::Controlq, desc_indexes);

        let response_type = unsafe { (*req).response.hdr.type_ };

        if response_type != VirtioGpuCtrlType::RespOkDisplayInfo.val() {
            panic!("virtio_gpu: get_display_info error {:?}", response_type);
        }

        unsafe { (*req).response }
    }

    // VIRTIO_GPU_CMD_RESOURCE_CREATE_2D
    // returns an resource id
    pub fn resource_create_2d(&mut self, width: u32, height: u32, format: VirtioGpuFormats) -> u32 {
        let req = Request::<VirtioGpuResourceCreate2d, VirtioGpuCtrlHdr>::new(
            VirtioGpuResourceCreate2d {
                hdr: VirtioGpuCtrlHdr {
                    type_: VirtioGpuCtrlType::CmdResourceCreate2d.val(),
                    flags: 0,
                    fence_id: 0,
                    ctx_id: 0,
                    padding: 0,
                },
                resource_id: self.resource_id,
                format: format.val(),
                width,
                height,
            },
        );
        let res = self.resource_id;

        let mut desc_indexes = Vec::new();
        self.allocate_desc(2, &mut desc_indexes);

        let desc = VirtqDesc::new(
            unsafe { &(*req).request as *const VirtioGpuResourceCreate2d as u64 },
            size_of::<VirtioGpuResourceCreate2d>() as u32,
            VirtqDescFlag::VirtqDescFNext.val(),
            desc_indexes[1],
        );
        self.write_desc(desc_indexes[0] as usize, VirtioGpuQueue::Controlq, desc);

        let desc = VirtqDesc::new(
            unsafe { &(*req).response as *const VirtioGpuCtrlHdr as u64 },
            size_of::<VirtioGpuCtrlHdr>() as u32,
            VirtqDescFlag::VirtqDescFWrite.val(),
            0,
        );
        self.write_desc(desc_indexes[1] as usize, VirtioGpuQueue::Controlq, desc);

        self.send_desc(VirtioGpuQueue::Controlq, desc_indexes);

        let response_type = unsafe { (*req).response.type_ };

        if response_type != VirtioGpuCtrlType::RespOkNodata.val() {
            panic!("virtio_gpu: resource_create_2d error {:?}", response_type);
        }

        res
    }

    pub fn resource_attach_backing(
        &mut self,
        width: u32,
        height: u32,
        pixel_size: u32,
        resource_id: u32,
    ) {
        let req =
            Request3::<VirtioGpuResourceAttachBacking, VirtioGpuMemEntry, VirtioGpuCtrlHdr>::new(
                VirtioGpuResourceAttachBacking {
                    hdr: VirtioGpuCtrlHdr {
                        type_: VirtioGpuCtrlType::CmdResourceAttachBacking.val(),
                        flags: 0,
                        fence_id: 0,
                        ctx_id: 0,
                        padding: 0,
                    },
                    resource_id: resource_id,
                    nr_entries: 1,
                },
                VirtioGpuMemEntry {
                    addr: self.framebuffer as u64,
                    length: width * pixel_size * height,
                    padding: 0,
                },
            );

        let mut desc_indexes = Vec::new();
        self.allocate_desc(3, &mut desc_indexes);

        let desc = VirtqDesc::new(
            unsafe { &(*req).request as *const VirtioGpuResourceAttachBacking as u64 },
            size_of::<VirtioGpuResourceAttachBacking>() as u32,
            VirtqDescFlag::VirtqDescFNext.val(),
            desc_indexes[1],
        );
        self.write_desc(desc_indexes[0] as usize, VirtioGpuQueue::Controlq, desc);

        let desc = VirtqDesc::new(
            unsafe { &(*req).mementries as *const VirtioGpuMemEntry as u64 },
            size_of::<VirtioGpuMemEntry>() as u32,
            VirtqDescFlag::VirtqDescFNext.val(),
            desc_indexes[2],
        );
        self.write_desc(desc_indexes[1] as usize, VirtioGpuQueue::Controlq, desc);

        let desc = VirtqDesc::new(
            unsafe { &(*req).response as *const VirtioGpuCtrlHdr as u64 },
            size_of::<VirtioGpuCtrlHdr>() as u32,
            VirtqDescFlag::VirtqDescFWrite.val(),
            0,
        );
        self.write_desc(desc_indexes[2] as usize, VirtioGpuQueue::Controlq, desc);

        self.send_desc(VirtioGpuQueue::Controlq, desc_indexes);

        let response_type = unsafe { (*req).response.type_ };

        if response_type != VirtioGpuCtrlType::RespOkNodata.val() {
            panic!(
                "virtio_gpu: resource_attach_backing error {:?}",
                response_type
            );
        }
    }

    pub fn set_scanout(&mut self, width: u32, height: u32, resource_id: u32) {
        let req = Request::<VirtioGpuSetScanout, VirtioGpuCtrlHdr>::new(VirtioGpuSetScanout {
            hdr: VirtioGpuCtrlHdr {
                type_: VirtioGpuCtrlType::CmdSetScanout.val(),
                flags: 0,
                fence_id: 0,
                ctx_id: 0,
                padding: 0,
            },
            r: VirtioGpuRect {
                x: 0,
                y: 0,
                width: width,
                height: height,
            },
            resource_id: resource_id,
            scanout_id: 0,
        });

        let mut desc_indexes = Vec::new();
        self.allocate_desc(2, &mut desc_indexes);

        let desc = VirtqDesc::new(
            unsafe { &(*req).request as *const VirtioGpuSetScanout as u64 },
            size_of::<VirtioGpuSetScanout>() as u32,
            VirtqDescFlag::VirtqDescFNext.val(),
            desc_indexes[1],
        );
        self.write_desc(desc_indexes[0] as usize, VirtioGpuQueue::Controlq, desc);

        let desc = VirtqDesc::new(
            unsafe { &(*req).response as *const VirtioGpuCtrlHdr as u64 },
            size_of::<VirtioGpuCtrlHdr>() as u32,
            VirtqDescFlag::VirtqDescFWrite.val(),
            0,
        );
        self.write_desc(desc_indexes[1] as usize, VirtioGpuQueue::Controlq, desc);

        self.send_desc(VirtioGpuQueue::Controlq, desc_indexes);

        let response_type = unsafe { (*req).response.type_ };

        if response_type != VirtioGpuCtrlType::RespOkNodata.val() {
            panic!("virtio_gpu: set_scanout error {:?}", response_type);
        }
    }

    pub fn transfer_to_host_2d(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        resource_id: u32,
    ) {
        let req = Request::<VirtioGpuTransferToHost2d, VirtioGpuCtrlHdr>::new(
            VirtioGpuTransferToHost2d {
                hdr: VirtioGpuCtrlHdr {
                    type_: VirtioGpuCtrlType::CmdTransferToHost2d.val(),
                    flags: 0,
                    fence_id: 0,
                    ctx_id: 0,
                    padding: 0,
                },
                r: VirtioGpuRect {
                    x: x,
                    y: y,
                    width: width,
                    height: height,
                },
                offset: 0,
                resource_id: resource_id,
                padding: 0,
            },
        );

        let mut desc_indexes = Vec::new();
        self.allocate_desc(2, &mut desc_indexes);

        let desc = VirtqDesc::new(
            unsafe { &(*req).request as *const VirtioGpuTransferToHost2d as u64 },
            size_of::<VirtioGpuTransferToHost2d>() as u32,
            VirtqDescFlag::VirtqDescFNext.val(),
            desc_indexes[1],
        );
        self.write_desc(desc_indexes[0] as usize, VirtioGpuQueue::Controlq, desc);

        let desc = VirtqDesc::new(
            unsafe { &(*req).response as *const VirtioGpuCtrlHdr as u64 },
            size_of::<VirtioGpuCtrlHdr>() as u32,
            VirtqDescFlag::VirtqDescFWrite.val(),
            0,
        );
        self.write_desc(desc_indexes[1] as usize, VirtioGpuQueue::Controlq, desc);

        self.send_desc(VirtioGpuQueue::Controlq, desc_indexes);

        let response_type = unsafe { (*req).response.type_ };

        if response_type != VirtioGpuCtrlType::RespOkNodata.val() {
            // panic!("virtio_gpu: transfer_to_host_2d error {:?}", response_type);
        }
    }

    pub fn resource_flush(&mut self, x: u32, y: u32, width: u32, height: u32, resource_id: u32) {
        let req =
            Request::<VirtioGpuResourceFlush, VirtioGpuCtrlHdr>::new(VirtioGpuResourceFlush {
                hdr: VirtioGpuCtrlHdr {
                    type_: VirtioGpuCtrlType::CmdResourceFlush.val(),
                    flags: 0,
                    fence_id: 0,
                    ctx_id: 0,
                    padding: 0,
                },
                r: VirtioGpuRect {
                    x: x,
                    y: y,
                    width: width,
                    height: height,
                },
                resource_id: resource_id,
                padding: 0,
            });

        let mut desc_indexes = Vec::new();
        self.allocate_desc(2, &mut desc_indexes);

        let desc = VirtqDesc::new(
            unsafe { &(*req).request as *const VirtioGpuResourceFlush as u64 },
            size_of::<VirtioGpuResourceFlush>() as u32,
            VirtqDescFlag::VirtqDescFNext.val(),
            desc_indexes[1],
        );
        self.write_desc(desc_indexes[0] as usize, VirtioGpuQueue::Controlq, desc);

        let desc = VirtqDesc::new(
            unsafe { &(*req).response as *const VirtioGpuCtrlHdr as u64 },
            size_of::<VirtioGpuCtrlHdr>() as u32,
            VirtqDescFlag::VirtqDescFWrite.val(),
            0,
        );
        self.write_desc(desc_indexes[1] as usize, VirtioGpuQueue::Controlq, desc);

        self.send_desc(VirtioGpuQueue::Controlq, desc_indexes);

        // println!("resource_flush type: {}", unsafe { (*req).response.type_ });

        let response_type = unsafe { (*req).response.type_ };

        if response_type != VirtioGpuCtrlType::RespOkNodata.val() {
            // panic!("virtio_gpu: resource_flush error {:?}", response_type);
        }
    }

    pub fn pending(&mut self) {
        // println!("virtio_gpu pending start");
        let mask = interrupt_disable();
        let interrupt_status = self.read_reg32(VirtioReg::InterruptStatus.val());
        self.write_reg32(VirtioReg::InterruptACK.val(), interrupt_status & 0x3);
        let virtqueue = unsafe { self.virtqueue[self.curr_queue as usize].as_mut() };
        let desc = virtqueue.desc;
        let used = unsafe { virtqueue.used.as_mut().unwrap() };
        while self.ack_used_index != used.idx {
            let index = self.ack_used_index % VIRTIO_RING_SIZE as u16;
            let elem = used.ring[index as usize];

            self.ack_used_index = self.ack_used_index.wrapping_add(1);
            unsafe {
                let desc = desc.add(elem.id as usize).as_mut().unwrap();
                let req_layout = Layout::from_size_align(desc.len as usize, 1).unwrap();
                let req = desc.addr as *mut u8;
                dealloc(req, req_layout);
            }
        }

        let desc_indexes = match self.desc_indexes {
            Some(ref indexes) => indexes.clone(),
            None => panic!("desc_indexes must be saved"),
        };
        self.deallocate_desc(&desc_indexes);

        let pm = unsafe { process_manager() };
        pm.io_signal(self.pid);
        pm.signal_semaphore(self.sid);

        interrupt_restore(mask);
        // println!("virtio_gpu pending end");
    }
}

pub fn init(base: usize) -> VirtioGpu {
    let mut gpu = VirtioGpu::new(base);
    gpu.init();
    gpu
}
