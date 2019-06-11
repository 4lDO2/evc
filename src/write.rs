use std::sync::Arc;
use std::sync::atomic;
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::{Epoch, Epochs, Inner, OperationCache, USIZE_MSB};

pub struct WriteHandle<T: OperationCache> {
    writers_inner: Arc<AtomicPtr<Inner<T>>>,
    readers_inner: Arc<AtomicPtr<Inner<T>>>,

    epochs: Epochs,
    last_epochs: Vec<usize>,

    ops: Vec<T::Operation>,
}

impl<T: OperationCache> WriteHandle<T> {
    pub(crate) fn new(writers_inner: Arc<AtomicPtr<Inner<T>>>, readers_inner: Arc<AtomicPtr<Inner<T>>>, epochs: Epochs) -> Self {
        Self {
            writers_inner,
            readers_inner,

            epochs,
            last_epochs: Vec::new(),
            ops: Vec::new(),
        }
    }
    pub fn write(&mut self, operation: T::Operation) {
        self.ops.push(operation)
    }
    fn wait(&mut self, epochs: &mut Vec<Epoch>) {
        self.last_epochs.resize(epochs.len(), 0);

        'retrying: loop {
            for (index, last_epoch) in self.last_epochs.iter().cloned().enumerate() {
                // Delete the reader from the epochs if the reader has dropped.
                if Arc::strong_count(&epochs[index]) == 1 {
                    epochs.remove(index);
                    self.last_epochs.remove(index);
                    continue 'retrying
                }

                let current_epoch = epochs[index].load(Ordering::Acquire);
                
                if current_epoch == last_epoch & current_epoch && USIZE_MSB == 0 && current_epoch != 0 {
                    continue 'retrying
                }
            }
            break
        }
    }
    pub fn refresh(&mut self) {
        let epochs = Arc::clone(&self.epochs);
        let mut epochs = epochs.lock().unwrap();
        self.wait(&mut epochs);

        unsafe {
            self.writers_inner.load(Ordering::Relaxed).as_mut().unwrap()
        }.value.apply_operations(self.ops.clone());

        // Swap the pointers.
        let writers_inner = self.writers_inner.swap(self.readers_inner.load(Ordering::Relaxed), Ordering::Release);
        self.readers_inner.store(writers_inner, Ordering::Release);

        atomic::fence(Ordering::SeqCst);

        for (i, epoch) in epochs.iter().enumerate() {
            self.last_epochs[i] = epoch.load(Ordering::Acquire);
        }

        unsafe {
            self.writers_inner.load(Ordering::Relaxed).as_mut().unwrap()
        }.value.apply_operations(self.ops.drain(0..self.ops.len()));
    }
}
