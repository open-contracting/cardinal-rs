// Adapted from https://github.com/dtolnay/oqueue
// Compared to oqueue, this implementation uses dynamic dispatch.

use std::collections::VecDeque;
use std::io::{Result, Write};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard, PoisonError};

pub struct Job<W: Write> {
    /// The number of tasks started.
    started: AtomicUsize,
    inner: Arc<Mutex<Inner<W>>>,
}

// Inner is always used inside an Arc<Mutex>.
struct Inner<W: Write> {
    /// The writer.
    writer: W,
    /// The number of outputs popped from the queue.
    popped: usize,
    /// A queue in which the index is the task's position (as an offset from `popped`), and the value is its output.
    pending: VecDeque<Output>,
}

/// A task's buffered output.
struct Output {
    /// The buffered output.
    buffer: Vec<u8>,
    /// Whether the task has dropped and can no longer write to the buffer.
    closed: bool,
}

#[readonly::make]
#[derive(Clone)]
pub struct Task<W: Write> {
    handle: Rc<Handle<W>>,
}

struct Handle<W: Write> {
    /// The number of previous tasks started.
    index: usize,
    inner: Arc<Mutex<Inner<W>>>,
}

/// A non-poisoning mutex.
struct Mutex<T: ?Sized> {
    std: StdMutex<T>,
}

impl<W: Write> Job<W> {
    pub fn new(writer: W) -> Self {
        Self {
            started: AtomicUsize::new(0),
            inner: Arc::new(Mutex::new(Inner {
                writer,
                popped: 0,
                pending: VecDeque::new(),
            })),
        }
    }

    pub fn new_task(&self) -> Task<W> {
        Task::new(self.started.fetch_add(1, Ordering::Relaxed), self.inner.clone())
    }
}

impl<W: Write> Inner<W> {
    fn get(&mut self, index: usize) -> &mut Output {
        assert!(index >= self.popped);

        let offset = index - self.popped;

        if offset >= self.pending.len() {
            self.pending.resize_with(offset + 1, || Output {
                buffer: vec![],
                closed: false,
            });
        }

        &mut self.pending[offset]
    }
}

impl Output {
    const fn is_closed(&self) -> bool {
        self.closed
    }
}

impl<W: Write> Task<W> {
    fn new(index: usize, inner: Arc<Mutex<Inner<W>>>) -> Self {
        Self {
            handle: Rc::new(Handle { index, inner }),
        }
    }

    fn apply<T>(&self, f: impl FnOnce(&mut dyn Write) -> T) -> T {
        let inner = &mut *self.handle.inner.lock();

        if self.handle.index == inner.popped {
            // If this task's buffer would be at the front of the queue, use the writer.
            f(&mut inner.writer)
        } else {
            // Otherwise, use the appropriate buffer in the queue.
            f(&mut inner.get(self.handle.index).buffer)
        }
    }
}

impl<W: Write> Write for Task<W> {
    fn write(&mut self, b: &[u8]) -> Result<usize> {
        self.apply(|w| w.write(b))
    }

    // https://github.com/rust-lang/rust-clippy/issues/9900
    #[allow(clippy::redundant_closure_for_method_calls)]
    fn flush(&mut self) -> Result<()> {
        self.apply(|w| w.flush())
    }
}

impl<W: Write> Drop for Handle<W> {
    fn drop(&mut self) {
        let inner = &mut *self.inner.lock();

        // Close this handle's buffer.
        inner.get(self.index).closed = true;

        // Write any closed buffers at the front of the queue.
        while inner.pending.front().is_some_and(Output::is_closed) {
            inner.popped += 1;
            let output = inner.pending.pop_front().unwrap();
            // https://github.com/BurntSushi/termcolor/blob/7f9c0307d774e04981312a2882e747d19ffdd9b2/src/lib.rs#L1023
            inner.writer.write_all(&output.buffer).unwrap();
        }

        // Write the content in the unclosed buffer at the front of the queue.
        if let Some(output) = inner.pending.get_mut(0) {
            inner.writer.write_all(&output.buffer).unwrap();
            output.buffer.clear();
        }
    }
}

impl<T> Mutex<T> {
    const fn new(value: T) -> Self {
        Self {
            std: StdMutex::new(value),
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    fn lock(&self) -> MutexGuard<'_, T> {
        self.std.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
