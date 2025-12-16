use conquer_once::spin::OnceCell;
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use crossbeam_queue::ArrayQueue;
use futures_util::stream::Stream;
use futures_util::task::AtomicWaker;

// A queue to hold scancodes. We use OnceCell for safe static initialization
static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();

// A waker to notify the executor when a new scancode arrives
static WAKER: AtomicWaker = AtomicWaker::new();

/// Called by the interrupt handler to push a scancode
pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if queue.push(scancode).is_err() {
            // Queue is full; drop the scancode (or log a warning)
            // In a real OS, you might signal a buffer overflow
            // println!("WARNING: Scancode queue full");
        } else {
            // Wake up the task waiting on this queue
            WAKER.wake();
        }
    } else {
        // Queue hasn't been initialized yet
        // println!("WARNING: Scancode queue uninitialized");
    }
}

pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE
            .try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE
            .try_get()
            .expect("Scancode queue not initialized");

        // Fast path: check if there's data already
        if let Some(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        // Slow path: register waker so we get notified later
        WAKER.register(cx.waker());

        // Check one more time to avoid a race condition (data arrived *while* registering)
        match queue.pop() {
            Some(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            None => Poll::Pending,
        }
    }
}
