// kernel/src/demo.rs
use crate::framebuffer::WRITER;
use core::future::Future;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use x86_64::instructions::interrupts; // Import interrupts

pub async fn bouncing_box() {
    let mut x = 100;
    let mut y = 100;
    let mut dx: isize = 4; // Horizontal speed
    let mut dy: isize = 4; // Vertical speed
    let size = 40;

    loop {
        // 1. Calculate new position
        // We scope this block so the Lock is released immediately after drawing!

        // SAFETY: We disable interrupts while locking the framebuffer.
        // If we don't, and a timer interrupt switches to the Shell while we hold this lock,
        // the Shell will try to lock it again to print "samux>", causing a Deadlock.
        interrupts::without_interrupts(|| {
            if let Some(writer) = WRITER.lock().as_mut() {
                let width = writer.width();
                let height = writer.height();

                // Clear previous box (draw black)
                writer.draw_rect(x, y, size, size, true);

                // Update position
                let next_x = (x as isize + dx) as usize;
                let next_y = (y as isize + dy) as usize;

                // Bounce X
                if next_x + size >= width || next_x <= 0 {
                    dx = -dx;
                }

                // Bounce Y
                if next_y + size >= height || next_y <= 0 {
                    dy = -dy;
                }

                x = (x as isize + dx) as usize;
                y = (y as isize + dy) as usize;

                // Draw new box (draw color - let's make it Blue)
                writer.set_color(0, 100, 255);
                writer.draw_rect(x, y, size, size, false);

                // Reset color to white for text
                writer.set_color(255, 255, 255);

                // === THE FIX ===
                // Push the changes from RAM to VRAM
                writer.present();
            }
        }); // Lock is released here, interrupts re-enabled

        // 2. Sleep/Yield
        for _ in 0..100 {
            yield_now().await;
        }
    }
}

// --- Helper for the sleep loop ---
struct YieldNow {
    yielded: bool,
}

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.yielded {
            Poll::Ready(())
        } else {
            self.yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

fn yield_now() -> YieldNow {
    YieldNow { yielded: false }
}
