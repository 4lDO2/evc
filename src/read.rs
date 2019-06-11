use std::cell::Cell;
use std::marker::PhantomData;
use std::mem;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crate::{Epoch, Epochs, Inner, USIZE_MSB};

/// A handle used for accessing data immutably using RAII guards.
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
        epochs.lock().unwrap().push(Arc::downgrade(&global_epoch));

        Self {
            inner,
            epochs,

            global_epoch,
            local_epoch: AtomicUsize::new(0),

            _not_sync: PhantomData,
        }
    }

    /// Create a RAII guard that allows reading the inner value directly.
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
    /// Crete a factory, used to make more read handles.
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
impl<T> Clone for ReadHandle<T> {
    fn clone(&self) -> Self{
        ReadHandle::new(Arc::clone(&self.inner), Arc::clone(&self.epochs))
    }
}

/// A factory for read handles, allows retrieving new `ReadHandle`s while still being `Sync`.
pub struct ReadHandleFactory<T> {
    inner: Arc<AtomicPtr<Inner<T>>>,
    epochs: Epochs,
}

impl<T> ReadHandleFactory<T> {
    /// Create a new handle.
    pub fn handle(&self) -> ReadHandle<T> {
        ReadHandle::new(Arc::clone(&self.inner), Arc::clone(&self.epochs))
    }
}

/// A RAII guard used to directly access the data of a read handle, immutably.
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
