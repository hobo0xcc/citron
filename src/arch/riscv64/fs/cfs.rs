use super::Disk;

pub struct FileSystem<'a, T: Disk> {
    disk: &'a mut T,
    buffer: [u8; 512],
}

impl<'a, T: Disk> FileSystem<'a, T> {
    pub fn new(disk: &'a mut T) -> Self {
        FileSystem {
            disk,
            buffer: [0; 512],
        }
    }

    pub fn fetch(&mut self, sector: usize) {
        self.disk.read_sector(sector, &mut self.buffer);
    }
}
