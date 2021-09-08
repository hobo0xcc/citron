use super::init::*;
use crate::*;

#[no_mangle]
pub unsafe extern "C" fn start() {
    init_all();
    println!("Hello, world!");
    loop {}
}