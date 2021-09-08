use super::super::virtio;
use super::super::virtio::*;
use crate::arch::riscv64::csr::Csr;
use crate::fs;
use crate::process::process_manager;
use alloc::alloc::dealloc;
use alloc::alloc::{alloc, alloc_zeroed};
use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::mem::size_of;
use core::ptr::NonNull;

#[derive(Copy, Clone)]
pub enum VirtioBlkFeature {
    VirtioBlkFSizeMax = 1 << 0,
    VirtioBlkFSegMax = 1 << 1,
    VirtioBlkFGeometry = 1 << 4,
    VirtioBlkFRo = 1 << 5,
    VirtioBlkFBlkSize = 1 << 6,
    VirtioBlkFFlush = 1 << 9,
    VirtioBlkFTopology = 1 << 10,
    VirtioBlkFConfigWce = 1 << 11,
    VirtioBlkFDiscard = 1 << 13,
    VirtioBlkFWriteZeroes = 1 << 14,
}

impl VirtioBlkFeature {
    pub fn val(&self) -> u32 {
        *self as u32
    }
}

#[derive(Copy, Clone)]
pub enum RequestType {
    VirtioBlkTIn = 0,
    VirtioBlkTOut = 1,
    VirtioBlkTFlush = 4,
    VirtioBlkTDiscard = 11,
    VirtioBlkTWriteZeroes = 13,
}

impl RequestType {
    pub fn val(&self) -> u32 {
        *self as u32
    }
}

#[repr(C, packed)]
#[allow(dead_code)]
#[derive(Copy, Clone)]
pub struct VirtioBlkRequest {
    type_: u32,
    _reserved: u32,
    sector: u64,
}

impl VirtioBlkRequest {
    pub fn new(type_: u32, sector: u64) -> Self {
        VirtioBlkRequest {
            type_,
            _reserved: 0,
            sector,
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct VirtioBlk {
    base: usize,
    virtqueue: NonNull<Virtqueue>,
    free_desc: [bool; VIRTIO_RING_SIZE], // true if the desc is free
    status: [u8; VIRTIO_RING_SIZE],
    desc_indexes: Option<Vec<u16>>,
    ack_used_index: u16,
    sid: usize, // Semaphore id
    pid: usize,
}

impl VirtioBlk {
    pub fn new(base: usize) -> VirtioBlk {
        let pm = unsafe { process_manager() };
        VirtioBlk {
            base,
            virtqueue: NonNull::dangling(),
            free_desc: [true; VIRTIO_RING_SIZE],
            status: [0; VIRTIO_RING_SIZE],
            desc_indexes: None,
            ack_used_index: 0,
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

    pub fn init(&mut self) {
        let pm = unsafe { process_manager() };

        pm.wait_semaphore(self.sid);

        let magic_value = self.read_reg32(VirtioReg::MagicValue.val());
        let version = self.read_reg32(VirtioReg::Version.val());
        let device_id = self.read_reg32(VirtioReg::DeviceId.val());
        if magic_value != 0x74726976 || version != 2 || device_id != 2 {
            panic!("unrecognized virtio device: {:#018x}", self.base);
        }

        let mut status_bits: u32 = 0;
        status_bits |= VirtioDeviceStatus::Acknowoledge.val();
        self.write_reg32(VirtioReg::Status.val(), status_bits);

        status_bits |= VirtioDeviceStatus::Driver.val();
        self.write_reg32(VirtioReg::Status.val(), status_bits);

        let mut features = self.read_reg32(VirtioReg::DeviceFeatures.val());
        features |= VirtioBlkFeature::VirtioBlkFConfigWce.val();
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

        self.write_reg32(VirtioReg::QueueNum.val(), VIRTIO_RING_SIZE as u32);

        self.write_reg64(VirtioReg::QueueDescLow.val(), desc as u64);
        self.write_reg64(VirtioReg::QueueDriverLow.val(), avail as u64);
        self.write_reg64(VirtioReg::QueueDeviceLow.val(), used as u64);
        self.virtqueue = NonNull::new(virtqueue).unwrap();

        self.write_reg32(VirtioReg::QueueReady.val(), 1);

        status_bits |= VirtioDeviceStatus::DriverOk.val();
        self.write_reg32(VirtioReg::Status.val(), status_bits);

        pm.signal_semaphore(self.sid);
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

    pub fn write_desc(&mut self, i: usize, desc: VirtqDesc) {
        unsafe {
            let desc_ptr = self.virtqueue.as_mut().desc.add(i);
            *desc_ptr = desc;
        }
    }

    pub fn capacity(&mut self) -> usize {
        self.read_reg64(0x100) as usize
    }

    pub fn block_op(&mut self, buffer: *mut u8, size: usize, sector: usize, write: bool) {
        let pm = unsafe { process_manager() };

        // until disk operation end
        pm.wait_semaphore(self.sid);

        self.pid = pm.running;

        let req_layout = Layout::from_size_align(size_of::<VirtioBlkRequest>(), 1).unwrap();
        let req = unsafe { alloc(req_layout) } as *mut VirtioBlkRequest;

        let req_type = if write {
            RequestType::VirtioBlkTOut.val()
        } else {
            RequestType::VirtioBlkTIn.val()
        };

        unsafe {
            *req = VirtioBlkRequest::new(req_type, sector as u64);
        }

        let mut desc_indexes = Vec::new();
        self.allocate_desc(3, &mut desc_indexes);

        // desc 0 (header)
        let flag = VirtqDescFlag::VirtqDescFNext.val();
        let desc = VirtqDesc::new(
            req as u64,
            size_of::<VirtioBlkRequest>() as u32,
            flag,
            desc_indexes[1],
        );
        self.write_desc(desc_indexes[0] as usize, desc);

        // desc 1 (buffer)
        let mut flag = VirtqDescFlag::VirtqDescFNext.val();
        if !write {
            flag |= VirtqDescFlag::VirtqDescFWrite.val();
        }
        let desc = VirtqDesc::new(buffer as u64, size as u32, flag, desc_indexes[2]);
        self.write_desc(desc_indexes[1] as usize, desc);

        // desc 2 (status)
        let flag = VirtqDescFlag::VirtqDescFWrite.val();
        let desc = VirtqDesc::new(
            (&mut self.status[desc_indexes[0] as usize]) as *mut u8 as u64,
            1,
            flag,
            0,
        );
        self.write_desc(desc_indexes[2] as usize, desc);

        pm.signal_semaphore(self.sid);

        unsafe {
            let mut avail = self.virtqueue.as_mut().avail.as_mut().unwrap();
            let index = avail.idx as usize;
            avail.ring[index % VIRTIO_RING_SIZE] = desc_indexes[0];
            asm!("fence iorw, iorw");
            avail.idx = avail.idx.wrapping_add(1);
            asm!("fence iorw, iorw");
            self.desc_indexes = Some(desc_indexes);
            pm.io_wait(self.pid);
            self.write_reg32(VirtioReg::QueueNotify.val(), 0);
            pm.schedule();
        }
    }

    #[allow(unaligned_references)]
    pub fn pending(&mut self) {
        let interrupt_status = self.read_reg32(VirtioReg::InterruptStatus.val());
        self.write_reg32(VirtioReg::InterruptACK.val(), interrupt_status & 0x3);
        let desc = unsafe { self.virtqueue.as_mut().desc };
        let used = unsafe { self.virtqueue.as_mut().used.as_mut().unwrap() };
        let mut freed_desc = BTreeSet::new();

        while self.ack_used_index != used.idx {
            let index = self.ack_used_index % VIRTIO_RING_SIZE as u16;
            let elem = used.ring[index as usize];
            if self.status[elem.id as usize] != 0 {
                println!("{}", self.status[elem.id as usize]);
                panic!("virtio_blk operation failed");
            }

            self.ack_used_index = self.ack_used_index.wrapping_add(1);
            unsafe {
                let desc = desc.add(elem.id as usize).as_mut().unwrap();
                let req_layout = Layout::from_size_align(desc.len as usize, 1).unwrap();
                let req = desc.addr as *mut u8;
                let req_val = req as usize;
                if freed_desc.contains(&req_val) {
                    continue;
                }
                freed_desc.insert(req_val);
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
    }
}

impl fs::Disk for VirtioBlk {
    fn read_sector(&mut self, sector: usize, buffer: &mut [u8]) {
        self.block_op(buffer.as_mut_ptr(), 512, sector, false);
    }

    fn write_sector(&mut self, sector: usize, buffer: &mut [u8]) {
        self.block_op(buffer.as_mut_ptr(), 512, sector, true);
    }

    fn sector_size(&self) -> usize {
        512
    }
}

pub fn init(base: usize) -> VirtioBlk {
    let mut blk = VirtioBlk::new(base);
    blk.init();
    blk
}
