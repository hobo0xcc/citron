use super::super::layout;
use crate::*;
use alloc::collections::BTreeMap;
use alloc::string::String;

// see qemu/docs/specs/fw_cfg.txt

// Selector Register address: Base + 8 (2 bytes)
// Data Register address:     Base + 0 (8 bytes)
// DMA Address address:       Base + 16 (8 bytes)
#[allow(dead_code)]
#[repr(packed)]
struct FwCfgRegs {
    data: usize,
    selector: u16,
    _padding0: u32,
    _padding1: u16,
    dma_addr: usize,
}

#[allow(dead_code)]
pub struct FwCfgFile {
    pub size: u32,
    pub select: u16,
}

#[allow(dead_code)]
pub struct FwCfg {
    base_addr: usize,
    regs: *mut FwCfgRegs,
}

#[allow(dead_code)]
impl FwCfg {
    pub fn new() -> FwCfg {
        FwCfg {
            base_addr: layout::_fw_cfg_start as usize,
            regs: layout::_fw_cfg_start as *mut FwCfgRegs,
        }
    }

    pub fn read_data8(&self) -> u8 {
        let data8 = self.base_addr as *mut u8;
        unsafe { *data8 }
    }

    pub fn read_data16(&self) -> u16 {
        let data16 = self.base_addr as *mut u16;
        unsafe { *data16 }
    }

    pub fn read_data32(&self) -> u32 {
        let data32 = self.base_addr as *mut u32;
        unsafe { *data32 }
    }

    pub fn read_data64(&self) -> u64 {
        let data64 = self.base_addr as *mut u64;
        unsafe { *data64 }
    }

    pub fn read_dma_addr(&self) -> u64 {
        let dma_addr = (self.base_addr + 16) as *mut u64;
        unsafe { *dma_addr }
    }

    pub fn set_selector(&mut self, selector: u16) {
        unsafe {
            // (*self.regs).selector = selector;
            let select = (self.base_addr + 8) as *mut u16;
            *select = selector;
        }
    }

    pub fn read_signature(&mut self) -> [u8; 4] {
        // FW_CFG_SIGNATURE
        self.set_selector(0x0000);
        let signature = self.read_data32();
        let dma_signature = self.read_dma_addr();
        println!("{:#018x}", dma_signature);

        signature.to_le_bytes()
    }

    pub fn read_feature_bitmap(&mut self) -> u32 {
        self.set_selector(0x0001);
        let feature_bitmap = self.read_data32();

        feature_bitmap
    }

    pub fn map_files(&mut self) -> BTreeMap<String, FwCfgFile> {
        // FW_CFG_FILE_DIR
        self.set_selector(0x0019);
        let count = self.read_data32();
        println!("{}", count);

        let mut map = BTreeMap::new();
        for _ in 0..count {
            let size = self.read_data32();
            let select = self.read_data16();
            let _reserved = self.read_data16();

            let mut name = String::new();
            for _ in 0..56 {
                let c = self.read_data8();
                if c == 0 {
                    continue;
                }
                name.push(c as char);
            }

            map.insert(name, FwCfgFile { size, select });
        }

        map
    }
}
