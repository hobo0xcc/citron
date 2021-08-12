#[derive(Copy, Clone)]
pub enum PagePerm {
    Read = 1,
    Write = 2,
    Exec = 3,
}

impl PagePerm {
    pub fn val(&self) -> usize {
        *self as usize
    }
}

pub trait Paging {
    fn map(vaddr: usize, paddr: usize, perm: usize);
    fn unmap(vaddr: usize);
}
