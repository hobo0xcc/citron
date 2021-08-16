use super::*;

pub fn init_all() {
    serial::init();
    plic::init();
    paging::init();
    virtio::init();
}
