//! OSCOMP mode entrypoint.
//!
//! When the `oscomp` feature is enabled, the kernel skips BusyBox init and
//! enters this path instead.

/// OSCOMP flow entry.
/// Replace this loop with the real oscomp-specific initialization when ready.
pub fn init() -> ! {
    crate::earlyprintln!("[OSCOMP] userland init skipped; entering oscomp mode");
    loop {
        crate::kernel::yield_task();
    }
}
