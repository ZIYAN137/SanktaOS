//! LoongArch64 self-tests
//!
//! 这些测试只在 `cfg(test)` 下编译，并由内核的测试运行器执行。

use loongArch64::register::{CpuMode, crmd};

use crate::kassert;

#[test_case]
fn test_loongarch_smoke() {
    // Basic CSR sanity: should be in PLV0 at boot.
    let plv = crmd::read().plv();
    kassert!(matches!(plv, CpuMode::Ring0));

    // Trap trampoline symbol should be linked in.
    let sigret = crate::arch::trap::sigreturn_trampoline_address();
    kassert!(sigret != 0);
    kassert!(sigret & 0x3 == 0);

    // Syscall numbers: asm-generic aligned sanity checks.
    kassert!(crate::arch::syscall::SYS_READ == 63);
    kassert!(crate::arch::syscall::SYS_WRITE == 64);
    kassert!(crate::arch::syscall::SYS_EXIT == 93);
}
