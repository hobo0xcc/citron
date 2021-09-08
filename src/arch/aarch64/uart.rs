use crate::arch::*;
use super::gpio::Reg;
use super::layout;

pub struct Uart {
    ptr: *mut u8,
}

impl Uart {
    pub fn new() -> Self {
        Uart { ptr: layout::MMIO_BASE as *mut u8 }
    }

    fn read_reg(&self, reg: Reg) -> u32 {
        unsafe {
            let addr = self.ptr.add(reg as usize) as *mut u32;
            addr.read_volatile()
        }
    }

    fn write_reg(&self, reg: Reg, val: u32) {
        unsafe {
            let addr = self.ptr.add(reg as usize) as *mut u32;
            addr.write_volatile(val);
        }
    }

    pub fn init(&mut self) {
        self.write_reg(Reg::AUX_ENABLE, self.read_reg(Reg::AUX_ENABLE) | 1);
        self.write_reg(Reg::AUX_MU_CNTL, 0);
        self.write_reg(Reg::AUX_MU_LCR, 3);
        self.write_reg(Reg::AUX_MU_MCR, 0);
        self.write_reg(Reg::AUX_MU_IER, 0);
        self.write_reg(Reg::AUX_MU_IIR, 0xc6);
        self.write_reg(Reg::AUX_MU_BAUD, 270);

        let mut r = self.read_reg(Reg::GPFSEL1);
        r &= !((7 << 12) | (7 << 15));
        r |= (2 << 12) | (2 << 15);
        self.write_reg(Reg::GPFSEL1, r);
        self.write_reg(Reg::GPPUD, 0);

        r = 150;
        while r > 0 {
            unsafe {
                asm!("nop");
            }
            r -= 1;
        }
        self.write_reg(Reg::GPPUDCLK0, (1 << 14) | (1 << 15));
        r = 150;
        while r > 0 {
            unsafe {
                asm!("nop");
            }
            r -= 1;
        }

        self.write_reg(Reg::GPPUDCLK0, 0);
        self.write_reg(Reg::AUX_MU_CNTL, 3);
    }
}

impl serial::SerialIO for Uart {
    fn put(&mut self, c: u8) {
        while (self.read_reg(Reg::AUX_MU_LSR) & 0x20) == 0 {
            unsafe {
                asm!("nop");
            }
        }
        self.write_reg(Reg::AUX_MU_IO, c as u32);
    }

    fn get(&mut self) -> Option<u8> {
        while (self.read_reg(Reg::AUX_MU_LSR) & 0x01) == 0 {
            unsafe {
                asm!("nop");
            }
        }

        let r = self.read_reg(Reg::AUX_MU_IO) as u8;
        if r == '\r' as u8 {
            Some('\n' as u8)
        } else {
            Some(r)
        }
    }
}