use super::csr::Csr;

pub fn timer_interrupt_on() {
    // enable supervisor software interrupt
    let mut sie_val = Csr::Sie.read();
    sie_val |= 1 << 1;
    Csr::Sie.write(sie_val);
}

pub fn timer_interrupt_off() {
    // disable supervisor software interrupt
    let mut sie_val = Csr::Sie.read();
    sie_val &= !(1 << 1);
    Csr::Sie.write(sie_val);
}

pub fn interrupt_off() {
    let mut sstatus = Csr::Sstatus.read();
    sstatus &= !(1 << 1); // unset SSTATUS.SIE
    Csr::Sstatus.write(sstatus);
}

pub fn interrupt_on() {
    let mut sstatus = Csr::Sstatus.read();
    sstatus |= 1 << 1; // set SSTATUS.SIE
    Csr::Sstatus.write(sstatus);
}

pub fn is_interrupt_enable() -> bool {
    let sstatus = Csr::Sstatus.read();
    let enable = (sstatus & 1 << 1) >> 1;
    enable != 0
}
