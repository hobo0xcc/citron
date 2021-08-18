pub mod cfs;

pub trait Disk {
    fn read_sector(&mut self, sector: usize, buffer: &mut [u8]);
    fn write_sector(&mut self, sector: usize, buffer: &mut [u8]);
}
