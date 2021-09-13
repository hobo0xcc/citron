use crate::process::*;
use core::ops::{Deref, DerefMut};
use spin::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub enum LockMutData<'a, T> {
    Locking(RwLockWriteGuard<'a, T>),
    SameProcess(&'a mut T),
}

pub enum LockData<'a, T> {
    Locking(RwLockReadGuard<'a, T>),
    SameProcess(&'a T),
}

pub struct LockMut<'a, T> {
    data: LockMutData<'a, T>,
    // mask: usize,
}
pub struct Lock<'a, T> {
    data: LockData<'a, T>,
    // mask: usize,
}

impl<'a, T> Drop for LockMut<'a, T> {
    fn drop(&mut self) {
        // interrupt_restore(self.mask);
        // let pm = unsafe { process_manager() };
        // pm.defer_schedule(DeferCommand::Stop).ok();
    }
}

impl<'a, T> Drop for Lock<'a, T> {
    fn drop(&mut self) {
        // interrupt_restore(self.mask);
        // let pm = unsafe { process_manager() };
        // pm.defer_schedule(DeferCommand::Stop).ok();
    }
}

impl<'a, T> Deref for LockMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self.data {
            LockMutData::Locking(ref lock) => lock,
            LockMutData::SameProcess(ref data) => &**data,
        }
    }
}

impl<'a, T> DerefMut for LockMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.data {
            LockMutData::Locking(ref mut lock) => lock,
            LockMutData::SameProcess(ref mut data) => &mut **data,
        }
    }
}

impl<'a, T> Deref for Lock<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self.data {
            LockData::Locking(ref lock) => lock,
            LockData::SameProcess(ref data) => &**data,
        }
    }
}

pub struct SpinLock<T> {
    spin_lock: RwLock<T>,
    recent_pid: Pid,
}

impl<T> SpinLock<T> {
    pub fn new(data: T) -> SpinLock<T> {
        SpinLock {
            spin_lock: RwLock::new(data),
            recent_pid: 0,
        }
    }

    pub fn lock_mut(&mut self) -> LockMut<T> {
        let pm = unsafe { process_manager() };
        // pm.defer_schedule(DeferCommand::Start).ok();
        // let mask = interrupt_disable();

        let lock = if pm.running == self.recent_pid {
            LockMut {
                data: LockMutData::SameProcess(self.spin_lock.get_mut()),
                // mask,
            }
        } else {
            LockMut {
                data: LockMutData::Locking(self.spin_lock.write()),
                // mask,
            }
        };

        self.recent_pid = pm.running;

        lock
    }

    pub fn lock(&mut self) -> Lock<T> {
        let pm = unsafe { process_manager() };
        // pm.defer_schedule(DeferCommand::Start).ok();
        // let mask = interrupt_disable();

        let lock = if pm.running == self.recent_pid {
            Lock {
                data: LockData::SameProcess(self.spin_lock.get_mut()),
                // mask,
            }
        } else {
            Lock {
                data: LockData::Locking(self.spin_lock.read()),
                // mask,
            }
        };

        self.recent_pid = pm.running;

        lock
    }

    pub fn get_inner_spinlock(&mut self) -> &RwLock<T> {
        &self.spin_lock
    }

    pub fn get_inner_spinlock_mut(&mut self) -> &mut RwLock<T> {
        &mut self.spin_lock
    }
}
