//! 设备驱动基础类型
//!
//! 包含 Driver trait 和 DeviceType 枚举

use alloc::{string::String, sync::Arc, vec::Vec};
use lazy_static::lazy_static;
use sync::RwLock;

use crate::block::BlockDriver;
use crate::net::NetDevice;
use crate::rtc::RtcDriver;
use crate::serial::SerialDriver;

/// 设备类型枚举
#[derive(Debug, Eq, PartialEq)]
pub enum DeviceType {
    /// 网络设备
    Net,
    /// 图形处理单元设备
    Gpu,
    /// 输入设备
    Input,
    /// 块设备
    Block,
    /// 实时时钟设备
    Rtc,
    /// 串行设备
    Serial,
    /// 中断控制器
    Intc,
}

/// 设备驱动程序特征
pub trait Driver: Send + Sync {
    /// 如果中断属于此驱动程序，则处理它并返回 true
    /// 否则返回 false
    /// 中断号在可用时提供
    /// 如果中断号不匹配，驱动程序应跳过处理。
    fn try_handle_interrupt(&self, irq: Option<usize>) -> bool;

    /// 返回对应的设备类型，请参阅 DeviceType
    fn device_type(&self) -> DeviceType;

    /// 获取此设备的唯一标识符
    /// 每个实例的标识符应该不同
    fn get_id(&self) -> String;

    /// 将驱动程序转换为网络驱动程序（如果适用）
    fn as_net(&self) -> Option<&dyn NetDevice> {
        None
    }

    /// 将驱动程序转换为网络驱动程序 Arc（如果适用）
    fn as_net_arc(self: Arc<Self>) -> Option<Arc<dyn NetDevice>> {
        None
    }

    /// 将驱动程序转换为块设备驱动程序（如果适用）
    fn as_block(&self) -> Option<&dyn BlockDriver> {
        None
    }

    /// 将驱动程序转换为块设备驱动程序 Arc（如果适用）
    fn as_block_arc(self: Arc<Self>) -> Option<Arc<dyn BlockDriver>> {
        None
    }

    /// 将驱动程序转换为实时时钟驱动程序（如果适用）
    fn as_rtc(&self) -> Option<&dyn RtcDriver> {
        None
    }

    /// 将驱动程序转换为实时时钟驱动程序 Arc（如果适用）
    fn as_rtc_arc(self: Arc<Self>) -> Option<Arc<dyn RtcDriver>> {
        None
    }

    /// 将驱动程序转换为串口驱动程序（如果适用）
    fn as_serial(&self) -> Option<&dyn SerialDriver> {
        None
    }
}

lazy_static! {
    // NOTE: RwLock 只在初始化阶段有写操作，运行时均为读操作
    /// 全局驱动列表
    pub static ref DRIVERS: RwLock<Vec<Arc<dyn Driver>>> = RwLock::new(Vec::new());
}

lazy_static! {
    /// 内核命令行参数
    /// 存储从设备树中提取的 bootargs 属性
    pub static ref CMDLINE: RwLock<String> = RwLock::new(String::new());
}

/// 注册设备驱动
pub fn register_driver(driver: Arc<dyn Driver>) {
    DRIVERS.write().push(driver);
}
