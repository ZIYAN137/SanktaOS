pub mod assert;
pub mod net_test;
use crate::arch::intr::{are_interrupts_enabled, disable_interrupts, enable_interrupts};

/// 可运行的测试用例。
///
/// 通过 `custom_test_frameworks`，编译器会收集所有带 `#[test_case]` 的函数并传入 `test_runner`。
/// 我们用一个 trait 包一层，便于：
/// - 在 runner 里统一输出测试名称
/// - 正确统计“失败测试数”与“失败断言数”
pub trait Testable {
    /// 运行测试，返回是否通过（本框架中：无失败断言即通过）。
    fn run(&self) -> bool;
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) -> bool {
        use core::any::type_name;
        use core::sync::atomic::Ordering;

        use crate::test::assert::{FAILED_LIST_CAPACITY, TEST_FAILED};

        crate::println!("\x1b[33m=======================================\x1b[0m");
        crate::println!("\x1b[33mRunning test: {}\x1b[0m", type_name::<T>());

        let failed_before = TEST_FAILED.load(Ordering::SeqCst);
        self();
        let failed_after = TEST_FAILED.load(Ordering::SeqCst);

        // 仅打印本测试新增的失败断言（对于直接 #[test_case] 的函数，也能看到失败细节）。
        unsafe {
            for i in failed_before..failed_after {
                if i >= FAILED_LIST_CAPACITY {
                    break;
                }
                // 避免创建对 `static mut` 的引用（Rust 2024 `static_mut_refs`）
                let base = core::ptr::addr_of!(crate::test::assert::FAILED_LIST)
                    as *const Option<crate::test::assert::FailedAssertion>;
                if let Some(fail) = base.add(i).read() {
                    crate::println!(
                        "\x1b[31mFailed assertion: {} at {}:{}\x1b[0m",
                        fail.cond,
                        fail.file,
                        fail.line
                    );
                }
            }
        }

        let failed_count = failed_after - failed_before;
        if failed_count == 0 {
            crate::println!("\x1b[32m[ok] Test passed\x1b[0m\n");
            true
        } else {
            crate::println!(
                "\x1b[91m[failed] Test failed with {} failed assertions\x1b[0m\n",
                failed_count
            );
            false
        }
    }
}

/// 测试运行器。它由测试框架自动调用，并传入一个包含所有测试的切片。
#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]) {
    use crate::arch::lib::sbi::shutdown;
    use crate::println;
    use crate::test::assert::TEST_FAILED;
    use crate::test::assert::{FAILED_INDEX, FAILED_LIST, FAILED_LIST_CAPACITY};
    use core::sync::atomic::Ordering;

    println!("\n\x1b[33m--- Running {} tests ---\x1b[0m", tests.len());

    // 重置失败计数器
    TEST_FAILED.store(0, Ordering::SeqCst);
    // 重置失败断言列表（避免跨次运行残留，且确保索引与 TEST_FAILED 对齐）
    unsafe {
        FAILED_INDEX = 0;
        // 避免创建对 `static mut` 的引用（Rust 2024 `static_mut_refs`）
        let base = core::ptr::addr_of_mut!(FAILED_LIST)
            as *mut Option<crate::test::assert::FailedAssertion>;
        for i in 0..FAILED_LIST_CAPACITY {
            base.add(i).write(None);
        }
    }

    // 遍历并执行所有测试
    let mut failed_tests: usize = 0;
    for test in tests {
        if !test.run() {
            failed_tests += 1;
        }
    }

    let failed_assertions = TEST_FAILED.load(Ordering::SeqCst);
    println!("\x1b[33m\n--- Test Summary ---\x1b[0m");
    println!(
        "\x1b[33mTotal: {}\x1b[0m, \x1b[32mPassed: {}\x1b[0m, \x1b[91mFailed: {}\x1b[0m (assertions: {})",
        tests.len(),
        tests.len() - failed_tests,
        failed_tests,
        failed_assertions
    );

    if failed_tests > 0 {
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
