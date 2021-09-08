use super::uart;
use crate::arch::serial;

impl serial::SerialInit for serial::Serial<uart::Uart> {
    fn init() -> Self {
        let dev = uart::Uart::new();
        serial::Serial {
            dev,
        }
    }
}

pub fn get_serial() -> serial::Serial<uart::Uart> {
    serial::Serial::<uart::Uart>::new()
}

pub fn init() {
    let mut dev = uart::Uart::new();
    dev.init();
}
