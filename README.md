# `evc`

A lock-free, eventually consistent synchronization primitive.

This primitive makes reading and writing possible at the same time, although refreshing is
needed to make writes visible to the readers.

This crate is very similar to [`evmap`](https://docs.rs/evmap), but generalized to any type
(evmap is a wrapper around HashMap). Unlike `evmap`, which wraps a HashMap, `evc` is lower
level, meaning that you need to be able to cache all possible mutations on the inner type
(`OperationCache`). Therefore making an extension trait and implementing it for
`WriteHandle<YourType>` is encouraged, so that accessing the inner data can be done using
regular methods (like `evmap` does internally).

# Examples

`VecWrapper`

```rust
use evc::OperationCache;

#[derive(Clone, Debug, Default)]
struct VecWrapper(Vec<u16>);

#[derive(Clone, Copy, Debug)]
enum Operation {
    Push(u16),
    Remove(usize),
    Clear,
}

impl OperationCache for VecWrapper {
    type Operation = Operation;

    fn apply_operation(&mut self, operation: Self::Operation) {
        match operation {
            Operation::Push(value) => self.0.push(value),
            Operation::Remove(index) => { self.0.remove(index); },
            Operation::Clear => self.0.clear(),
        }
    }
}

let (mut w_handle, r_handle) = evc::new(VecWrapper::default());

w_handle.write(Operation::Push(42));
w_handle.write(Operation::Push(24));

assert_eq!(r_handle.read().0, &[]);

w_handle.refresh();

assert_eq!(r_handle.read().0, &[42, 24]);

w_handle.write(Operation::Push(55));
w_handle.write(Operation::Remove(0));
w_handle.refresh();

assert_eq!(r_handle.read().0, &[24, 55]);

w_handle.write(Operation::Clear);

assert_eq!(r_handle.read().0, &[24, 55]);

w_handle.refresh();

assert_eq!(r_handle.read().0, &[]);

```
