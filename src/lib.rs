#![deny(missing_docs)]

//! A lock-free, eventually consistent synchronization primitive.
//!
//! This primitive makes reading and writing possible at the same time, although refreshing is
//! needed to make writes visible to the readers.
//!
//! This crate is very similar to [`evmap`](https://docs.rs/evmap), but generalized to any type
//! (evmap is a wrapper around HashMap). Unlike `evmap`, which wraps a HashMap, `evc` is lower
//! level, meaning that you need to be able to cache all possible mutations on the inner type
//! (`OperationCache`). Therefore making an extension trait and implementing it for
//! `WriteHandle<YourType>` is encouraged, so that accessing the inner data can be done using
//! regular methods (like `evmap` does internally).

use std::mem;
use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::{AtomicPtr, AtomicUsize};

mod read;
pub use read::{ReadHandle, ReadHandleFactory, ReadHandleGuard};

mod write;
pub use write::WriteHandle;

pub(crate) type Epoch = Arc<AtomicUsize>;
pub(crate) type WeakEpoch = Weak<AtomicUsize>;
pub(crate) type Epochs = Arc<Mutex<Vec<WeakEpoch>>>;

/// Represents anything that can be mutated using operations. This trait has to be implemented in
/// order to store it in an `evc`.
pub trait OperationCache {
    /// The operation this type uses for modifying itself.
    type Operation: Clone;

    /// Apply a series of operations on this type.
    fn apply_operations<O>(&mut self, operations: O)
    where
        O: IntoIterator<Item = Self::Operation>;
}

pub(crate) struct Inner<T> {
    value: T,
}

pub(crate) const USIZE_MSB: usize = 1 << (mem::size_of::<usize>() * 8 - 1);

/// Create a write handle and a read handle to some data. The data must be both `OperationCache`,
/// to support queuing data (so that both buffers can be modified during refreshes), and `Clone`,
/// to make double buffering possible.
pub fn new<T: Clone + OperationCache>(value: T) -> (WriteHandle<T>, ReadHandle<T>)
{
    let readers_inner = Arc::new(AtomicPtr::new(Box::into_raw(Box::new(Inner { value: value.clone() }))));
    let writers_inner = Arc::new(AtomicPtr::new(Box::into_raw(Box::new(Inner { value }))));

    let epochs = Arc::new(Mutex::new(Vec::new()));

    let read_handle = ReadHandle::new(Arc::clone(&readers_inner), Arc::clone(&epochs));
    let write_handle = WriteHandle::new(writers_inner, readers_inner, epochs);

    (write_handle, read_handle)
}
