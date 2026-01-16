//! IrqOps trait 实现

use device::IrqOps;

struct IrqOpsImpl;

impl IrqOps for IrqOpsImpl {
    fn enable_irq(&self, irq: usize) {
        crate::arch::intr::enable_irq(irq);
    }
}

static IRQ_OPS: IrqOpsImpl = IrqOpsImpl;

/// 初始化设备操作
pub fn init_device_ops() {
    // SAFETY: 在单线程启动阶段调用
    unsafe {
        device::register_irq_ops(&IRQ_OPS);
    }
}
