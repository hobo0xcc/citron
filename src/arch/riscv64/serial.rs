use super::uart;
use crate::arch::serial;

impl serial::SerialInit for serial::Serial<uart::Uart> {
    fn init() -> Self {
        serial::Serial {
            dev: uart::Uart::new(),
        }
    }
}

pub fn get_serial() -> serial::Serial<uart::Uart> {
    serial::Serial::<uart::Uart>::new()
}

pub fn init() {
    uart::init();
}
