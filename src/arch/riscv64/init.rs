use super::*;

pub fn init_all() {
    serial::init();
    paging::init();
}
