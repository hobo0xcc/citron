use core::fmt::{Error, Write};

pub trait SerialIO {
    fn put(&mut self, c: u8);
    fn get(&mut self) -> Option<u8>;
}

pub trait SerialInit {
    fn init() -> Self;
}

pub struct Serial<T>
where
    T: SerialIO,
{
    pub dev: T,
}

impl<T> Serial<T>
where
    T: SerialIO,
    Serial<T>: SerialInit,
{
    pub fn new() -> Self {
        Self::init()
    }
}

impl<T> Write for Serial<T>
where
    T: SerialIO,
{
    fn write_str(&mut self, out: &str) -> Result<(), Error> {
        for c in out.bytes() {
            self.dev.put(c);
        }

        Ok(())
    }
}
