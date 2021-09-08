extern "C" {
    pub fn __text_start();
    pub fn __text_end();
    pub fn __rodata_start();
    pub fn __rodata_end();
    pub fn __data_start();
    pub fn __data_end();
    pub fn __bss_start();
    pub fn __bss_end();
    pub fn _heap_start();
    pub fn _heap_end();
}

pub const MMIO_BASE: usize = 0x3F000000;