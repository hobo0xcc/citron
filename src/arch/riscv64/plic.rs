use super::layout;

#[derive(Copy, Clone)]
pub enum PlicReg {
    Senable = 0x2080,
    Spriority = 0x201000,
    Sclaim = 0x201004,
}

impl PlicReg {
    pub fn val(&self) -> usize {
        *self as usize
    }
}

#[derive(Copy, Clone)]
pub enum Irq {
    VirtioFirstIrq = 1,
    VirtioEndIrq = 8,
    UartIrq = 10,
}

impl Irq {
    pub fn val(&self) -> usize {
        *self as usize
    }
}

pub fn read_reg32(offset: usize) -> u32 {
    let base = layout::_plic_start as usize;
    let ptr = (base + offset) as *mut u32;
    unsafe { ptr.read_volatile() }
}

pub fn write_reg32(offset: usize, val: u32) {
    let base = layout::_plic_start as usize;
    let ptr = (base + offset) as *mut u32;
    unsafe {
        ptr.write_volatile(val);
    }
}

pub fn claim() -> u32 {
    read_reg32(PlicReg::Sclaim.val())
}

pub fn complete(irq: u32) {
    write_reg32(PlicReg::Sclaim.val(), irq);
}

pub extern "C" fn init() {
    // uart priority
    write_reg32(Irq::UartIrq.val() * 4, 1);
    // virtio priority
    // TODO: add more virtio devices
    write_reg32(Irq::VirtioFirstIrq.val() * 4, 1);
    write_reg32((Irq::VirtioFirstIrq.val() + 1) * 4, 1);
    write_reg32((Irq::VirtioFirstIrq.val() + 2) * 4, 1);
    write_reg32((Irq::VirtioFirstIrq.val() + 3) * 4, 1);

    let senable = (1 << Irq::UartIrq.val())
        | (1 << Irq::VirtioFirstIrq.val())
        | (1 << (Irq::VirtioFirstIrq.val() + 1))
        | (1 << (Irq::VirtioFirstIrq.val() + 2))
        | (1 << (Irq::VirtioFirstIrq.val() + 3));
    write_reg32(PlicReg::Senable.val(), senable);

    write_reg32(PlicReg::Spriority.val(), 0);
}
