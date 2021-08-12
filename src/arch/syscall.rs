pub trait SysCallInfo {
    fn get_arg_raw(&self, idx: usize) -> usize;
    fn get_arg_ptr<T>(&self, idx: usize) -> *mut T;
}
