use super::paging::*;
use crate::fs::*;
use crate::*;
use alloc::alloc::alloc_zeroed;
use alloc::format;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::slice;
use goblin::Object;

type EntryPoint = usize;

#[derive(Clone)]
#[allow(dead_code)]
pub struct ExecutableInfo {
    pub entry: EntryPoint,
    pub segment_buffers: Vec<(*mut u8, Layout)>,
}

impl ExecutableInfo {
    pub fn new() -> Self {
        ExecutableInfo {
            entry: 0,
            segment_buffers: Vec::new(),
        }
    }
}

pub fn load_exe(path: &str, page_table: &mut Table) -> Result<ExecutableInfo, Error> {
    let fs = unsafe { file_system() };
    let fd = fs.open_file(path)?;
    let size = fs.get_file_size(fd)?;

    let layout = Layout::from_size_align(size, 0x1000).unwrap();
    let bin_data = unsafe { alloc_zeroed(layout) };
    let bin_slice = unsafe { slice::from_raw_parts_mut(bin_data, size) };
    fs.read(fd, bin_slice)?;

    let obj = Object::parse(bin_slice).unwrap();
    let elf = match obj {
        Object::Elf(elf) => elf,
        _ => return Err(Error::Msg(format!("{} is not an elf file", path))),
    };

    let mut segment_buffers = Vec::new();

    for ph in elf.program_headers.iter() {
        let vm_range = ph.vm_range();
        let layout = Layout::from_size_align(vm_range.len(), 0x1000).unwrap();
        let buffer = unsafe { alloc_zeroed(layout) };
        map_range(
            page_table,
            vm_range.start,
            buffer as usize,
            vm_range.len(),
            EntryBits::R.val() | EntryBits::W.val() | EntryBits::X.val() | EntryBits::U.val(),
        );

        let range = ph.file_range();
        unsafe {
            buffer
                .copy_from_nonoverlapping(bin_slice[range.start..range.end].as_ptr(), range.len());
        }

        segment_buffers.push((buffer, layout));
    }

    // for section in elf.section_headers.iter() {
    //     let vm_range = section.vm_range();
    //     let layout = Layout::from_size_align(vm_range.len(), 0x1000).unwrap();
    //     let mut buffer = unsafe { alloc_zeroed(layout) };
    //     println!("{:#018x}", vm_range.start);
    //     map_range(
    //         page_table,
    //         vm_range.start,
    //         buffer as usize,
    //         vm_range.len() as usize,
    //         EntryBits::R.val() | EntryBits::W.val() | EntryBits::X.val() | EntryBits::U.val(),
    //     );

    //     if let Some(range) = section.file_range() {
    //         unsafe {
    //             println!("addr: {:#018x}", vm_range.start as usize);
    //             println!("range.start: {:02x}", bin_slice[range.start]);
    //             buffer
    //                 .add(vm_range.start as usize - (vm_range.start as usize & !0x0fff_usize))
    //                 .copy_from_nonoverlapping(
    //                     bin_slice[range.start..range.end].as_ptr(),
    //                     range.len(),
    //                 );
    //         }
    //     }

    //     section_buffers.push((buffer, layout));
    // }

    let exec_info = ExecutableInfo {
        entry: elf.header.e_entry as usize,
        segment_buffers,
    };

    Ok(exec_info)
}
