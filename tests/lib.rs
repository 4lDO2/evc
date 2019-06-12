extern crate evc;

use std::mem;
use std::thread;

use evc::OperationCache;

// A simple struct with only push operations.
#[derive(Clone, Debug, Default)]
struct VecWrapper(Vec<u16>);

#[derive(Clone, Copy, Debug)]
struct Push(u16);

impl OperationCache for VecWrapper {
    type Operation = Push;

    fn apply_operation(&mut self, operation: Self::Operation) {
        self.0.push(operation.0)
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

#[test]
fn multithreaded() {
    let mut threads = Vec::with_capacity(10);
    let n = 0xBEEF;

    let (mut w_handle, r_handle) = evc::new(VecWrapper::default());

    for _ in 0..10 {
        let r_handle = r_handle.clone();
        threads.push(thread::spawn(move || {
            for index in 0..n {
                'retrying: loop {
                    match r_handle.read().0.get(index as usize) {
                        Some(&num) => {
                            assert_eq!(num, index);
                            break 'retrying
                        },
                        None => thread::yield_now(),
                    }
                }
            }
        }));
    }

    for index in 0..n {
        w_handle.write(Push(index));
        w_handle.refresh();
    }

    for thread in threads {
        thread.join().unwrap();
    }
}

#[test]
fn write_after_drop() {
    let (mut w_handle, r_handle) = evc::new(VecWrapper::default());

    w_handle.write(Push(0));
    w_handle.refresh();

    assert_eq!(r_handle.read().0, &[0]);

    mem::drop(r_handle);
    
    w_handle.write(Push(1));
    w_handle.refresh();
}
