pub mod fat;

use crate::arch::riscv64::virtio::virtio_blk::*;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::string::ToString;

pub static mut FS: Option<FileSystem<fat::Fat32<VirtioBlk>>> = None;

pub trait Disk {
    fn read_sector(&mut self, sector: usize, buffer: &mut [u8]);
    fn write_sector(&mut self, sector: usize, buffer: &mut [u8]);
    fn sector_size(&self) -> usize;
}

#[derive(Debug)]
pub enum Error {
    Msg(String),
    FileNotOpen,
    FileNotExist,
}

pub trait BackingFileSystem {
    fn read_at(&mut self, buffer: &mut [u8], path: &str, offset: usize) -> Result<usize, Error>;
    fn file_size(&mut self, path: &str) -> Result<usize, Error>;
}

type FileDesc = usize;

pub struct File {
    pub path: String,
    pub fd: FileDesc,
    pub offset: usize,
    pub size: usize,
}

impl File {
    pub fn new(path: &str, fd: FileDesc, size: usize) -> Self {
        File {
            path: path.to_string(),
            fd,
            offset: 0,
            size,
        }
    }
}

pub struct FileSystem<'a, T: BackingFileSystem> {
    backing: &'a mut T,
    desc_table: BTreeMap<FileDesc, File>,
    curr_fd: FileDesc,
}

impl<'a, T: BackingFileSystem> FileSystem<'a, T> {
    pub fn new(backing: &'a mut T) -> Self {
        FileSystem {
            backing,
            desc_table: BTreeMap::new(),
            curr_fd: 1,
        }
    }

    pub fn open_file(&mut self, path: &str) -> Result<FileDesc, Error> {
        let fd = self.curr_fd;
        self.curr_fd += 1;
        let size = self.backing.file_size(path)?;
        let file = File::new(path, fd, size);
        self.desc_table.insert(fd, file);
        Ok(fd)
    }

    pub fn get_file_size(&mut self, fd: FileDesc) -> Result<usize, Error> {
        Ok(self.desc_table.get(&fd).ok_or(Error::FileNotOpen)?.size)
    }

    pub fn read(&mut self, fd: FileDesc, buffer: &mut [u8]) -> Result<(), Error> {
        let file = self.desc_table.get(&fd).ok_or(Error::FileNotOpen)?;
        self.backing
            .read_at(buffer, file.path.as_str(), file.offset)?;
        Ok(())
    }
}

pub unsafe fn file_system() -> &'static mut FileSystem<'static, fat::Fat32<'static, VirtioBlk>> {
    match FS {
        Some(ref mut fs) => fs,
        None => panic!("file system is uninitialized"),
    }
}

pub fn init() {
    let fs = FileSystem::new(unsafe { fat::fat32() });
    unsafe {
        FS = Some(fs);
    }
}
