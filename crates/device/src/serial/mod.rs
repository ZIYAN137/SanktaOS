//! 串行设备驱动模块
//!
//! 包含串行设备驱动程序的接口定义

use alloc::{sync::Arc, vec::Vec};
use lazy_static::lazy_static;
use sync::RwLock;

use crate::driver::Driver;

lazy_static! {
    /// 全局串行设备驱动列表
    pub static ref SERIAL_DRIVERS: RwLock<Vec<Arc<dyn SerialDriver>>> = RwLock::new(Vec::new());
}

/// 串行设备驱动程序特征
pub trait SerialDriver: Driver {
    /// 从 tty 读取一个字节
    fn read(&self) -> u8;

    /// 向 tty 写入数据
    fn write(&self, data: &[u8]);

    /// 尝试读取一个字节，如果没有数据则返回 None
    fn try_read(&self) -> Option<u8> {
        Some(self.read())
    }
}
