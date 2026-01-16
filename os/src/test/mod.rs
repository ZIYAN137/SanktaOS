pub mod guard;
pub mod macros;
pub mod net_test;
use crate::arch::intr::{are_interrupts_enabled, disable_interrupts, enable_interrupts};

/// 测试运行器。它由测试框架自动调用，并传入一个包含所有测试的切片。
#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    use crate::arch::lib::sbi::shutdown;
    use crate::println;
    use crate::test::macros::TEST_FAILED;
    use core::sync::atomic::Ordering;

    println!("\n\x1b[33m--- Running {} tests ---\x1b[0m", tests.len());

    // 重置失败计数器
    TEST_FAILED.store(0, Ordering::SeqCst);

    // 遍历并执行所有测试
    for test in tests {
        test();
    }

    let failed = TEST_FAILED.load(Ordering::SeqCst);
    println!("\x1b[33m\n--- Test Summary ---\x1b[0m");
    println!(
        "\x1b[33mTotal: {}\x1b[0m, \x1b[32mPassed: {}\x1b[0m, \x1b[91mFailed: {}\x1b[0m",
        tests.len(),
        tests.len() - failed,
        failed
    );

    if failed > 0 {
        println!("\x1b[91mSome tests failed!\x1b[0m");
        shutdown(true);
    } else {
        println!("\x1b[32mAll tests passed!\x1b[0m");
        shutdown(false);
    }
}

/// 一个 RAII 守卫，用于在作用域内启用中断，并在离开作用域时恢复之前的状态。
///
/// # Safety
///
/// 创建此守卫需要调用 `enable_interrupts`，这是一个 `unsafe` 操作。
/// 因此，`new` 函数也是 `unsafe` 的。封装它的宏将负责处理安全性。
pub struct InterruptGuard {
    // 存储创建守卫之前的中断状态，以便恢复。
    // true = 已启用, false = 已禁用
    was_enabled: bool,
}

impl InterruptGuard {
    /// 创建一个新的守卫并启用中断。
    ///
    /// # Safety
    ///
    /// 调用者必须确保此时启用中断是安全的。例如，不能在持有自旋锁时调用。
    #[inline(always)]
    pub fn new() -> Self {
        // 读取当前中断状态,保存，如果是禁用的drop时会重新禁用
        let was_enabled = are_interrupts_enabled();
        // 启用中断
        unsafe {
            enable_interrupts();
        }
        Self { was_enabled }
    }
}

impl Drop for InterruptGuard {
    #[inline(always)]
    fn drop(&mut self) {
        if !self.was_enabled {
            // 如果之前是禁用的，就再次禁用。
            unsafe {
                disable_interrupts();
            }
        }
        // 如果之前是启用的，我们什么都不用做，因为中断本来就是开启的。
    }
}
