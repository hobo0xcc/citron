use crate::arch::target::*;
use alloc::alloc::Layout;
use linked_list_allocator::LockedHeap;

#[global_allocator]
static GLOBAL: LockedHeap = LockedHeap::empty();

pub unsafe fn init() {
    let heap_start = layout::_heap_start as usize;
    let heap_end = layout::_heap_end as usize;
    let heap_size = heap_end - heap_start;
    GLOBAL.lock().init(heap_start, heap_size);
}

#[alloc_error_handler]
fn on_oom(_layout: Layout) -> ! {
    loop {}
}
