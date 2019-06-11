use std::cell::Cell;
use std::marker::PhantomData;
use std::mem;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crate::{Epoch, Epochs, Inner, USIZE_MSB};

pub struct ReadHandle<T> {
    inner: Arc<AtomicPtr<Inner<T>>>,
    epochs: Epochs,

    global_epoch: Epoch,
    local_epoch: AtomicUsize,

    _not_sync: PhantomData<Cell<()>>,
}
impl<T> ReadHandle<T> {
    pub(crate) fn new(inner: Arc<AtomicPtr<Inner<T>>>, epochs: Epochs) -> Self {
        let global_epoch = Arc::new(AtomicUsize::new(0));
        epochs.lock().unwrap().push(Arc::clone(&global_epoch));

        Self {
            inner,
            epochs,

            global_epoch,
            local_epoch: AtomicUsize::new(0),

            _not_sync: PhantomData,
        }
    }

    pub fn read(&'_ self) -> ReadHandleGuard<'_, T> {
        let epoch = self.local_epoch.fetch_add(1, Ordering::Relaxed) + 1;
        self.global_epoch.store(epoch, Ordering::Release);

        atomic::fence(Ordering::SeqCst);

        let pointer = self.inner.load(Ordering::Acquire);

        ReadHandleGuard {
            handle: self,
            pointer,
            epoch,
        }
    }
    pub fn factory(&self) -> ReadHandleFactory<T> {
        ReadHandleFactory {
            inner: Arc::clone(&self.inner),
            epochs: Arc::clone(&self.epochs),
        }
    }
}
impl<T> Drop for ReadHandle<T> {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            let readers_inner = self.inner.load(Ordering::Relaxed);
            mem::drop(unsafe { Box::from_raw(readers_inner) });
        }
    }
}

pub struct ReadHandleFactory<T> {
    inner: Arc<AtomicPtr<Inner<T>>>,
    epochs: Epochs,
}

impl<T> ReadHandleFactory<T> {
    pub fn handle(&self) -> ReadHandle<T> {
        ReadHandle::new(Arc::clone(&self.inner), Arc::clone(&self.epochs))
    }
}


pub struct ReadHandleGuard<'a, T> {
    handle: &'a ReadHandle<T>,
    epoch: usize,
    pointer: *const Inner<T>,
}
impl<T> Deref for ReadHandleGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &(*self.pointer).value }
    }
}
impl<T> Drop for ReadHandleGuard<'_, T> {
    fn drop(&mut self) {
        self.handle.global_epoch.store(self.epoch | USIZE_MSB, Ordering::Release);
    }
}
