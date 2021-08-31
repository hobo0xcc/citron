use crate::arch::target::interrupt::*;
use crate::arch::target::*;
use alloc::alloc::GlobalAlloc;
use alloc::alloc::Layout;
use core::ptr::NonNull;
use linked_list_allocator::LockedHeap;

pub struct Allocator {
    backing: LockedHeap,
}

impl Allocator {
    pub const fn new(backing: LockedHeap) -> Self {
        Allocator { backing }
    }
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mask = interrupt_disable();
        let ptr = self
            .backing
            .lock()
            .allocate_first_fit(layout)
            .ok()
            .map_or(0 as *mut u8, |allocation| allocation.as_ptr());
        interrupt_restore(mask);
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mask = interrupt_disable();
        self.backing
            .lock()
            .deallocate(NonNull::new_unchecked(ptr), layout);
        interrupt_restore(mask);
    }
}

#[global_allocator]
static GLOBAL: Allocator = Allocator::new(LockedHeap::empty());

pub unsafe fn init() {
    let heap_start = layout::_heap_start as usize;
    let heap_end = layout::_heap_end as usize;
    let heap_size = heap_end - heap_start;
    GLOBAL.backing.lock().init(heap_start, heap_size);
}

#[alloc_error_handler]
fn on_oom(_layout: Layout) -> ! {
    loop {}
}
