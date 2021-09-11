pub mod fat;

use core::mem::MaybeUninit;

use crate::arch::riscv64::virtio::virtio_blk::*;
use crate::*;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::string::ToString;
use spin::Mutex;

pub static mut FS: MaybeUninit<Mutex<FileSystem<fat::Fat32<VirtioBlk>>>> = MaybeUninit::uninit();

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
    UnknownOption,
}

pub trait BackingFileSystem {
    fn read_at(&mut self, buffer: &mut [u8], path: &str, offset: usize) -> Result<usize, Error>;
    fn file_size(&mut self, path: &str) -> Result<usize, Error>;
}

#[allow(non_camel_case_types)]
#[repr(u32)]
pub enum SeekWhence {
    SEEK_SET = 0,
    SEEK_CUR = 1,
    SEEK_END = 2,
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
            curr_fd: 3,
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

    pub fn seek(&mut self, fd: FileDesc, offset: isize, whence: u32) -> Result<usize, Error> {
        if whence == SeekWhence::SEEK_SET as u32 {
            let file = self.desc_table.get_mut(&fd).ok_or(Error::FileNotOpen)?;

            file.offset = offset as usize;
            return Ok(file.offset);
        } else if whence == SeekWhence::SEEK_CUR as u32 {
            let file = self.desc_table.get_mut(&fd).ok_or(Error::FileNotOpen)?;

            let mut offset_isize = file.offset as isize;
            offset_isize += offset;
            file.offset = offset_isize as usize;
            return Ok(file.offset);
        } else if whence == SeekWhence::SEEK_END as u32 {
            let file = self.desc_table.get_mut(&fd).ok_or(Error::FileNotOpen)?;

            let mut offset_isize = file.size as isize;
            offset_isize += offset;
            file.offset = offset_isize as usize;
            return Ok(file.offset);
        } else {
            return Err(Error::UnknownOption);
        }
    }

    pub fn read(&mut self, fd: FileDesc, buffer: &mut [u8]) -> Result<usize, Error> {
        let file = self.desc_table.get_mut(&fd).ok_or(Error::FileNotOpen)?;
        let size = self
            .backing
            .read_at(buffer, file.path.as_str(), file.offset)?;
        file.offset += size;
        Ok(size)
    }
}

pub unsafe fn file_system(
) -> &'static mut Mutex<FileSystem<'static, fat::Fat32<'static, VirtioBlk>>> {
    FS.assume_init_mut()
    // match FS {
    //     Some(ref mut fs) => fs,
    //     None => panic!("file system is uninitialized"),
    // }
}

pub fn init() {
    let dev = unsafe { fat::fat32() };
    let fs = FileSystem::new(dev.get_mut());
    unsafe {
        FS = MaybeUninit::new(Mutex::new(fs));
    }
}
