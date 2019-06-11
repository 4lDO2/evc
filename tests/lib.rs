extern crate evc;

use std::mem;

use evc::OperationCache;

// A simple struct with only push operations.
#[derive(Clone, Debug, Default)]
struct VecWrapper(Vec<u16>);

#[derive(Clone, Copy, Debug)]
struct Push(u16);

impl OperationCache for VecWrapper {
    type Operation = Push;

    fn apply_operations<O: IntoIterator<Item = Self::Operation>>(&mut self, operations: O) {
        for operation in operations {
            self.0.push(operation.0)
        }
    }
}

#[test]
fn basic_sync_operations() {
    let (mut w_handle, r_handle) = evc::new(VecWrapper::default());

    w_handle.write(Push(57));
    w_handle.write(Push(94));

    assert_eq!(r_handle.read().0, &[]);
    w_handle.refresh();
    assert_eq!(r_handle.read().0, &[57, 94]);

    w_handle.write(Push(42));
    assert_eq!(r_handle.read().0, &[57, 94]);
    w_handle.refresh();
    assert_eq!(r_handle.read().0, &[57, 94, 42]);
}

#[test]
fn read_after_drop() {
    let (mut w_handle, r_handle) = evc::new(VecWrapper::default());

    w_handle.write(Push(1337));
    mem::drop(w_handle);

    assert_eq!(r_handle.read().0, &[1337]);
}

// TODO: Write more tests.
