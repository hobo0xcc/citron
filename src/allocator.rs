use crate::arch::target::*;
use alloc::alloc::Layout;
use core::alloc::GlobalAlloc;
use core::cell::UnsafeCell;
use core::ptr;

// ref: https://tomoyuki-nakabayashi.github.io/embedded-rust-techniques/03-bare-metal/allocator.html

#[global_allocator]
pub static mut GLOBAL: SimpleAllocator = SimpleAllocator {
    head: UnsafeCell::new(0x0),
    end: 0x0,
};

pub unsafe fn init() {
    *GLOBAL.head.get() = layout::_heap_start as usize;
    GLOBAL.end = layout::_heap_end as usize;
}

pub struct SimpleAllocator {
    head: UnsafeCell<usize>,
    end: usize,
}

unsafe impl Sync for SimpleAllocator {}

unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let head = self.head.get();
        let align = layout.align();
        let res = *head % align;
        let start = if res == 0 { *head } else { *head + align - res };
        if start + align > self.end {
            ptr::null_mut()
        } else {
            *head = start + layout.size();
            start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {
        // nothing to do
    }
}

#[alloc_error_handler]
fn on_oom(_layout: Layout) -> ! {
    loop {}
}
