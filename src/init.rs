use crate::*;

pub unsafe fn init_all() {
    allocator::init();
    process::init();
    arch::target::init::init_all();
    fs::fat::init();
    fs::init();
}
