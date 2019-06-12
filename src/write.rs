use std::mem;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::thread;

use crate::{WeakEpoch, Epochs, Inner, OperationCache, USIZE_MSB};

/// A handle which allows accessing the inner data mutably through operations.
pub struct WriteHandle<T: OperationCache> {
    writers_inner: Option<Arc<AtomicPtr<Inner<T>>>>,
    readers_inner: Arc<AtomicPtr<Inner<T>>>,

    epochs: Epochs,
    last_epochs: Vec<usize>,

    ops: Vec<T::Operation>,
}

impl<T: OperationCache> WriteHandle<T> {
    pub(crate) fn new(writers_inner: Arc<AtomicPtr<Inner<T>>>, readers_inner: Arc<AtomicPtr<Inner<T>>>, epochs: Epochs) -> Self {
        Self {
            writers_inner: Some(writers_inner),
            readers_inner,

            epochs,
            last_epochs: Vec::new(),
            ops: Vec::new(),
        }
    }
    /// Mutate the inner data using an operation.
    pub fn write(&mut self, operation: T::Operation) {
        self.ops.push(operation)
    }
    fn wait(&mut self, epochs: &mut Vec<WeakEpoch>) {
        let mut start_index = 0;
        let mut retry_count = 0;

        self.last_epochs.resize(epochs.len(), 0);

        'retrying: loop {
            for index in start_index..self.last_epochs.len() {
                // Delete the reader from the epochs if the reader has dropped.
                let epoch = match epochs[index].upgrade() {
                    Some(e) => e,
                    None => {
                        epochs.remove(index);
                        self.last_epochs.remove(index);

                        // TODO: Maybe this "garbage collecting could happen in another loop?
                        start_index = 0;
                        continue 'retrying
                    }
                };

                if self.last_epochs[index] & USIZE_MSB != 0 {
                    continue
                }

                let current_epoch = epoch.load(Ordering::Acquire);
                
                if current_epoch == self.last_epochs[index] && current_epoch & USIZE_MSB == 0 && current_epoch != 0 {
                    start_index = index;

                    if retry_count < 32 {
                        retry_count += 1;
                    } else {
                        thread::yield_now();
                    }

                    continue 'retrying
                }
            }
            break
        }
    }
    /// Refresh the queued writes, making the changes visible to readers.
    pub fn refresh(&mut self) {
        let epochs = Arc::clone(&self.epochs);
        let mut epochs = epochs.lock().unwrap();
        self.wait(&mut epochs);

        let w_handle = &mut unsafe {
            self.writers_inner.as_ref().unwrap().load(Ordering::Relaxed).as_mut().unwrap()
        }.value;

        for operation in self.ops.iter().cloned() {
            w_handle.apply_operation(operation);
        }

        // Swap the pointers.
        let writers_inner = self.writers_inner.as_ref().unwrap().swap(self.readers_inner.load(Ordering::Relaxed), Ordering::Release);
        self.readers_inner.store(writers_inner, Ordering::Release);

        atomic::fence(Ordering::SeqCst);

        for (i, epoch) in epochs.iter().enumerate() {
            if let Some(e) = epoch.upgrade() {
                self.last_epochs[i] = e.load(Ordering::Acquire);
            }
        }

        let w_handle = &mut unsafe {
            self.writers_inner.as_ref().unwrap().load(Ordering::Relaxed).as_mut().unwrap()
        }.value;

        for operation in self.ops.drain(0..self.ops.len()) {
            w_handle.apply_operation(operation)
        }
    }
    /// Consume this writer to retrieve the inner value.
    pub fn into_inner(mut self) -> T {
        let writers_inner = self.writers_inner.take().unwrap();
        unsafe { Box::from_raw(writers_inner.swap(ptr::null_mut(), Ordering::Relaxed)) }.value
    }
}

impl<T: OperationCache> Drop for WriteHandle<T> {
    fn drop(&mut self) {
        if self.writers_inner.is_some() {
            if !self.ops.is_empty() {
                self.refresh();
            }
            assert!(self.ops.is_empty());

            let writers_inner = self.writers_inner.as_ref().unwrap().swap(ptr::null_mut(), Ordering::Relaxed);
            mem::drop(unsafe { Box::from_raw(writers_inner) });
        }

        // The readers should be able to continue reading after this writer has gone, and thus they
        // should be responsible for destroying their handle.
    }
}
