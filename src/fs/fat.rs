use crate::arch::riscv64::virtio::block_device;
use crate::process::process_manager;
use crate::*;
use alloc::alloc::alloc_zeroed;
use alloc::alloc::dealloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cmp::min;
use core::slice::from_raw_parts_mut;

use super::*;

#[cfg(target_arch = "riscv64")]
use arch::riscv64::virtio::virtio_blk::*;
#[cfg(target_arch = "riscv64")]
pub static mut FAT32_FS: Option<Fat32<VirtioBlk>> = None;

// https://wiki.osdev.org/FAT

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct DirEntry {
    file_name: [u8; 11],
    attr: u8,
    _reserved: u8,
    creation_time_sec: u8,
    creation_time: u16,
    creation_date: u16,
    last_access_date: u16,
    first_cluster_high: u16,
    last_mod_time: u16,
    last_mod_date: u16,
    first_cluster_low: u16,
    size: u32,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct LFNEntry {
    order: u8,
    name1: [u16; 5],
    attr: u8,
    lfn_type: u8,
    checksum: u8,
    name2: [u16; 6],
    _zero: u16,
    name3: [u16; 2],
}

#[derive(Copy, Clone)]
#[allow(non_snake_case)]
#[repr(C, packed)]
pub struct Fat32BootSector {
    BS_jmpBoot: [u8; 3],
    BS_OEMName: [u8; 8],
    BPB_BytsPerSec: u16,
    BPB_SecPerClus: u8,
    BPB_RsvdSecCnt: u16,
    BPB_NumFATs: u8,
    BPB_RootEntCnt: u16,
    BPB_TotSec16: u16,
    BPB_Media: u8,
    BPB_FATSz16: u16,
    BPB_SecPerTrk: u16,
    BPB_NumHeads: u16,
    BPB_HiddSec: u32,
    BPB_TotSec32: u32,
    BPB_FATSz32: u32,
    BPB_ExtFlags: u16,
    BPB_FSVer: u16,
    BPB_RootClus: u32,
    BPB_FSInfo: u16,
    BPB_BkBootSec: u16,
    BPB_Reserved: [u8; 12],
    BS_DrvNum: u8,
    BS_Reserved1: u8,
    BS_BootSig: u8,
    BS_VolID: u32,
    BS_VolLab: [u8; 11],
    BS_FilSysType: [u8; 8],
}

pub struct Fat32<'a, T: Disk> {
    disk: &'a mut T,
    buffer: [u8; 512],
    fat_begin: u32,
    cluster_begin: u32,
    sectors_per_cluster: u8,
    root_dir_first_cluster: u32,
    sector_size: u32,
    sid: usize,
}

impl<'a, T: Disk> Fat32<'a, T> {
    pub fn new(disk: &'a mut T) -> Self {
        let pm = unsafe { process_manager() };
        Fat32 {
            disk,
            buffer: [0; 512],
            fat_begin: 0,
            cluster_begin: 0,
            sectors_per_cluster: 0,
            root_dir_first_cluster: 0,
            sector_size: 512,
            sid: pm.create_semaphore(1),
        }
    }

    pub unsafe fn init(&mut self) {
        let pm = process_manager();
        pm.wait_semaphore(self.sid).expect("process");

        self.read_bootsector();
        let bs = *(self.buffer.as_mut_ptr() as *mut Fat32BootSector);

        let fat_start_sector = bs.BPB_RsvdSecCnt as u32;
        let fat_sectors = bs.BPB_FATSz32 * bs.BPB_NumFATs as u32;
        let root_dir_start_sector = fat_start_sector + fat_sectors;
        let root_dir_first_cluster = bs.BPB_RootClus;
        let fatsz = if bs.BPB_FATSz16 != 0 {
            bs.BPB_FATSz16 as u32
        } else {
            bs.BPB_FATSz32
        };
        let totsec = if bs.BPB_TotSec16 != 0 {
            bs.BPB_TotSec16 as u32
        } else {
            bs.BPB_TotSec32
        };
        let root_dir_sectors =
            ((bs.BPB_RootEntCnt * 32) + (bs.BPB_BytsPerSec - 1)) / bs.BPB_BytsPerSec;
        let data_sec = totsec
            - (bs.BPB_RsvdSecCnt as u32
                + (bs.BPB_NumFATs as u32 * fatsz)
                + root_dir_sectors as u32);
        let count_of_clusters = data_sec / bs.BPB_SecPerClus as u32;

        if count_of_clusters < 4085 {
            // fat12
            unimplemented!();
        } else if count_of_clusters < 65525 {
            // fat16
            unimplemented!();
        } else {
            // fat32
            // do nothing
        }

        self.fat_begin = bs.BPB_RsvdSecCnt as u32;
        self.cluster_begin = root_dir_start_sector;
        self.sectors_per_cluster = bs.BPB_SecPerClus;
        self.root_dir_first_cluster = root_dir_first_cluster;
        self.sector_size = bs.BPB_BytsPerSec as u32;

        pm.signal_semaphore(self.sid).expect("process");
    }

    #[allow(unaligned_references)]
    pub fn lfn_to_string(&self, lfn: LFNEntry) -> String {
        let mut res = String::new();
        for ch in lfn.name1.iter() {
            if *ch == 0xffff {
                continue;
            }
            if *ch == 0 {
                continue;
            }
            if (*ch as u8 as char).is_whitespace() {
                continue;
            }
            res.push(*ch as u8 as char);
        }
        for ch in lfn.name2.iter() {
            if *ch == 0xffff {
                continue;
            }
            if *ch == 0 {
                continue;
            }
            if (*ch as u8 as char).is_whitespace() {
                continue;
            }
            res.push(*ch as u8 as char);
        }
        for ch in lfn.name3.iter() {
            if *ch == 0xffff {
                continue;
            }
            if *ch == 0 {
                continue;
            }
            if (*ch as u8 as char).is_whitespace() {
                continue;
            }
            res.push(*ch as u8 as char);
        }

        res
    }

    pub fn dir_entry_name(&self, entry: DirEntry) -> String {
        let mut res = String::new();
        for ch in entry.file_name[0..8].iter() {
            if (*ch as char).is_whitespace() {
                continue;
            }
            res.push(*ch as char);
        }

        let mut ext = String::new();
        for ch in entry.file_name[8..11].iter() {
            if (*ch as char).is_whitespace() {
                continue;
            }

            ext.push(*ch as char);
        }

        if ext.len() == 0 {
            return res;
        } else {
            res.push('.');
            res.push_str(ext.as_str());
            return res;
        }
    }

    pub unsafe fn find_file(&mut self, mut dir_cluster: u32, file_name: &str) -> Option<DirEntry> {
        let mut curr_idx = 0;
        let layout = Layout::from_size_align(512, 8).unwrap();
        let buffer = alloc_zeroed(layout);
        let buf_slice = from_raw_parts_mut(buffer, 512);

        let mut found = false;

        self.read_cluster(dir_cluster, buf_slice);

        loop {
            let mut entry = buffer.add(curr_idx * 32);
            let is_end = entry.read() == 0x00;
            let _is_unused = entry.read() == 0xE5;
            if is_end {
                break;
            }

            let is_lfn = entry.add(11).read() == 0x0F;
            if is_lfn {
                let lfn_entry = entry as *mut LFNEntry;
                let name = self.lfn_to_string(*lfn_entry);
                if name == file_name {
                    found = true;
                }
                curr_idx += 1;

                if curr_idx >= 16 {
                    let next_cluster = self.next_cluster(dir_cluster);
                    if let Some(cluster) = next_cluster {
                        self.read_cluster(cluster, buf_slice);
                        dir_cluster = cluster;
                    } else {
                        break;
                    }
                    curr_idx %= 16;
                }

                entry = buffer.add(curr_idx * 32);
            }

            let dir_entry = entry as *mut DirEntry;
            if found {
                dealloc(buffer, layout);
                return Some(*dir_entry);
            } else {
                let name = self.dir_entry_name(*dir_entry);
                if name == file_name.to_uppercase() {
                    dealloc(buffer, layout);
                    return Some(*dir_entry);
                }
            }
            curr_idx += 1;

            if curr_idx >= 16 {
                let next_cluster = self.next_cluster(dir_cluster);
                if let Some(cluster) = next_cluster {
                    self.read_cluster(cluster, buf_slice);
                    dir_cluster = cluster;
                } else {
                    break;
                }
                curr_idx %= 16;
            }
        }

        dealloc(buffer, layout);
        None
    }

    pub fn get_entry_from_path(&mut self, path: &str) -> Option<DirEntry> {
        let mut cluster = self.root_dir_first_cluster;
        let mut curr_entry = None;
        let split_path: Vec<&str> = path.split('/').collect();
        for name in split_path.into_iter() {
            if name.len() == 0 {
                continue;
            }
            if let Some(entry) = unsafe { self.find_file(cluster, name) } {
                cluster =
                    (entry.first_cluster_high as u32) << 16 | (entry.first_cluster_low as u32);
                curr_entry = Some(entry);
            } else {
                return None;
            }
        }

        curr_entry
    }

    pub unsafe fn list_all_files_in_dir(&mut self, mut cluster_num: u32) {
        let mut curr_idx = 0;
        let layout = Layout::from_size_align(512, 8).unwrap();
        let buffer = alloc_zeroed(layout);
        let buf_slice = from_raw_parts_mut(buffer, 512);

        self.read_cluster(cluster_num, buf_slice);
        loop {
            let mut entry = buffer.add(curr_idx * 32);
            let is_end = entry.read() == 0x00;
            let is_unused = entry.read() == 0xE5;
            if is_end || is_unused {
                break;
            }

            let is_lfn = entry.add(11).read() == 0x0F;
            if is_lfn {
                let lfn_entry = entry as *mut LFNEntry;
                let name = self.lfn_to_string(*lfn_entry);
                println!("{}", name);
                curr_idx += 1;

                if curr_idx >= 128 {
                    let next_cluster = self.next_cluster(cluster_num);
                    if let Some(cluster) = next_cluster {
                        self.read_cluster(cluster, buf_slice);
                        cluster_num = cluster;
                    } else {
                        break;
                    }
                    curr_idx %= 128;
                }

                entry = buffer.add(curr_idx * 32);
            }

            let dir_entry = entry as *mut DirEntry;
            if !is_lfn {
                let name = self.dir_entry_name(*dir_entry);
                println!("{}", name);
            }
            curr_idx += 1;

            if curr_idx >= 128 {
                let next_cluster = self.next_cluster(cluster_num);
                if let Some(cluster) = next_cluster {
                    self.read_cluster(cluster, buf_slice);
                    cluster_num = cluster;
                } else {
                    break;
                }
                curr_idx %= 128;
            }
        }

        dealloc(buffer, layout);
    }

    fn read_sector(&mut self, sector_num: u32, buffer: &mut [u8]) {
        for i in 0..(self.sector_size as usize / self.disk.sector_size()) {
            self.disk.read_sector(sector_num as usize + i, buffer);
        }
    }

    pub fn read_cluster(&mut self, cluster_num: u32, buffer: &mut [u8]) {
        let first_sector = self.sector_of_cluster(cluster_num);
        for i in 0..self.sectors_per_cluster as u32 {
            self.read_sector(
                first_sector + i,
                &mut buffer[(i * self.sector_size) as usize..],
            );
        }
    }

    fn sector_of_cluster(&self, cluster_num: u32) -> u32 {
        ((cluster_num - 2) * self.sectors_per_cluster as u32) + self.cluster_begin
    }

    fn next_cluster(&mut self, cluster_num: u32) -> Option<u32> {
        let fat_offset = cluster_num * 4;
        let fat_sector = self.fat_begin + (fat_offset / self.sector_size);
        let ent_offset = fat_offset % self.sector_size;
        self.disk
            .read_sector(fat_sector as usize, &mut self.buffer[0..]);
        let table_value = unsafe {
            (self.buffer.as_ptr().add(ent_offset as usize) as *const u32).read() & 0x0FFFFFFF
        };

        if table_value >= 0x0FFFFFF8 {
            // no cluster in the chain
            None
        } else if table_value == 0x0FFFFFF7 {
            // bad cluster
            panic!("bad cluster");
        } else {
            Some(table_value)
        }
    }

    unsafe fn read_bootsector(&mut self) {
        self.disk.read_sector(0, &mut self.buffer[0..]);
    }
}

impl<'a, T: Disk> BackingFileSystem for Fat32<'a, T> {
    fn read_at(&mut self, buffer: &mut [u8], path: &str, offset: usize) -> Result<usize, Error> {
        let entry = self
            .get_entry_from_path(path)
            .ok_or(Error::Msg(format!("file does not exist: {}", path)))?;

        let mut cluster = (entry.first_cluster_high as u32) << 16 | entry.first_cluster_low as u32;
        let cluster_size = self.sector_size * self.sectors_per_cluster as u32;
        let offset_cluster = offset as u32 / cluster_size;
        let mut offset_byte = offset as u32 % cluster_size;

        let mut read_bytes = 0;
        let mut read_clusters = 0;

        let layout = Layout::from_size_align(
            self.sector_size as usize * self.sectors_per_cluster as usize,
            8,
        )
        .unwrap();
        let tmp_buf = unsafe { alloc_zeroed(layout) };
        while read_bytes < buffer.len() {
            if read_clusters < offset_cluster {
                read_clusters += 1;
                if let Some(clus_num) = self.next_cluster(cluster) {
                    cluster = clus_num;
                } else {
                    return Ok(read_bytes);
                }
                continue;
            }

            self.read_cluster(cluster, unsafe {
                from_raw_parts_mut(tmp_buf, cluster_size as usize)
            });

            let count = min(
                (cluster_size - offset_byte) as usize,
                buffer.len() - read_bytes,
            );
            unsafe {
                buffer[read_bytes..]
                    .as_mut_ptr()
                    .copy_from_nonoverlapping(tmp_buf.add(offset_byte as usize), count);
            }

            read_bytes += count;
            offset_byte = 0;
            read_clusters += 1;

            if let Some(clus_num) = self.next_cluster(cluster) {
                cluster = clus_num;
            } else {
                return Ok(read_bytes);
            }
        }

        unsafe {
            dealloc(tmp_buf, layout);
        }

        Ok(read_bytes)
    }

    fn file_size(&mut self, path: &str) -> Result<usize, Error> {
        let entry = self
            .get_entry_from_path(path)
            .ok_or(Error::Msg(format!("file does not exist: {}", path)))?;
        Ok(entry.size as usize)
    }
}

#[cfg(target_arch = "riscv64")]
pub unsafe fn fat32() -> &'static mut Fat32<'static, VirtioBlk> {
    match FAT32_FS {
        Some(ref mut fat) => fat,
        None => panic!("fat32 is uninitialized"),
    }
}

pub fn init() {
    #[cfg(target_arch = "riscv64")]
    let mut fat32 = Fat32::<VirtioBlk>::new(unsafe { block_device() });
    unsafe {
        fat32.init();
        FAT32_FS = Some(fat32);
    }
}
