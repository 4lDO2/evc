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

pub trait OperationCache {
    type Operation: Clone;

    fn apply_operations<O>(&mut self, operations: O)
    where
        O: IntoIterator<Item = Self::Operation>;
}

pub(crate) struct Inner<T> {
    value: T,
}

pub(crate) const USIZE_MSB: usize = 1 << (mem::size_of::<usize>() * 8 - 1);

pub fn new<T: Clone + OperationCache>(value: T) -> (WriteHandle<T>, ReadHandle<T>)
{
    let readers_inner = Arc::new(AtomicPtr::new(Box::into_raw(Box::new(Inner { value: value.clone() }))));
    let writers_inner = Arc::new(AtomicPtr::new(Box::into_raw(Box::new(Inner { value }))));

    let epochs = Arc::new(Mutex::new(Vec::new()));

    let read_handle = ReadHandle::new(Arc::clone(&readers_inner), Arc::clone(&epochs));
    let write_handle = WriteHandle::new(writers_inner, readers_inner, epochs);

    (write_handle, read_handle)
}

#[derive(Debug, Clone)]
struct S(Vec<u16>);

impl OperationCache for S {
    type Operation = u16;

    fn apply_operations<O>(&mut self, operations: O) where O: IntoIterator<Item = Self::Operation> {
        for operation in operations {
            self.0.push(operation);
        }
    }
}

#[test]
fn test() {
    let vector = S(vec! [1337, 42, 1445, 5494]);
    let (mut w_handle, r_handle) = new(vector);
    w_handle.write(24);
    w_handle.refresh();

    println!("{:?}", r_handle.read().0);

    w_handle.write(38);
    w_handle.write(69);
    w_handle.refresh();
    println!("{:?}", r_handle.read().0);
}
