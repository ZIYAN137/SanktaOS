//! 测试支持 crate
//!
//! 提供测试运行器、Mock 实现和测试工具

#![no_std]

pub mod mock;

/// 测试运行器
pub fn test_runner(tests: &[&dyn Fn()]) {
    for test in tests {
        test();
    }
}
