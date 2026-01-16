//! 设备注册表辅助 - 为 sysfs 提供设备信息访问

use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use device::block::BlockDriver;
use device::rtc::RtcDriver;
use device::{BLK_DRIVERS, DRIVERS, DeviceType, NetDevice};

/// 块设备信息 (用于 sysfs)
#[derive(Clone)]
pub struct BlockDeviceInfo {
    pub name: String,
    pub major: u32,
    pub minor: u32,
    pub device: Arc<dyn BlockDriver>,
}

/// 网络设备信息 (用于 sysfs)
#[derive(Clone)]
pub struct NetworkDeviceInfo {
    pub name: String,
    pub ifindex: u32,
    pub device: Arc<dyn NetDevice>,
}

/// TTY 设备信息 (用于 sysfs)
#[derive(Clone)]
pub struct TtyDeviceInfo {
    pub name: String,
    pub major: u32,
    pub minor: u32,
}

/// 输入设备信息 (用于 sysfs)
#[derive(Clone)]
pub struct InputDeviceInfo {
    pub name: String,
    pub id: u32,
}

/// RTC 设备信息 (用于 sysfs)
#[derive(Clone)]
pub struct RtcDeviceInfo {
    pub name: String,
    pub id: u32,
    pub device: Arc<dyn RtcDriver>,
}

/// 列出所有块设备
pub fn list_block_devices() -> Vec<BlockDeviceInfo> {
    let drivers = BLK_DRIVERS.read();
    drivers
        .iter()
        .enumerate()
        .map(|(idx, driver)| {
            let name = format!("vd{}", (b'a' + idx as u8) as char);
            let major = 254;
            let minor = idx as u32;

            BlockDeviceInfo {
                name,
                major,
                minor,
                device: driver.clone(),
            }
        })
        .collect()
}

/// 列出所有网络设备
pub fn list_net_devices() -> Vec<NetworkDeviceInfo> {
    let drivers = DRIVERS.read();
    drivers
        .iter()
        .filter(|driver| driver.device_type() == DeviceType::Net)
        .enumerate()
        .filter_map(|(idx, driver)| {
            Arc::clone(driver)
                .as_net_arc()
                .map(|net_dev| NetworkDeviceInfo {
                    name: net_dev.name().to_string(),
                    ifindex: (idx + 1) as u32,
                    device: net_dev,
                })
        })
        .collect()
}

/// 根据名称查找块设备
pub fn find_block_device(name: &str) -> Option<BlockDeviceInfo> {
    list_block_devices()
        .into_iter()
        .find(|dev| dev.name == name)
}

/// 根据名称查找网络设备
pub fn find_net_device(name: &str) -> Option<NetworkDeviceInfo> {
    list_net_devices().into_iter().find(|dev| dev.name == name)
}

/// 列出所有 TTY 设备
pub fn list_tty_devices() -> Vec<TtyDeviceInfo> {
    let drivers = DRIVERS.read();
    let mut ttys = Vec::new();

    ttys.push(TtyDeviceInfo {
        name: "console".to_string(),
        major: 5,
        minor: 1,
    });

    ttys.push(TtyDeviceInfo {
        name: "tty0".to_string(),
        major: 4,
        minor: 0,
    });

    let serial_count = drivers
        .iter()
        .filter(|driver| driver.device_type() == DeviceType::Serial)
        .count();

    for idx in 0..serial_count {
        ttys.push(TtyDeviceInfo {
            name: format!("ttyS{}", idx),
            major: 4,
            minor: (64 + idx) as u32,
        });
    }

    ttys
}

/// 列出所有输入设备
pub fn list_input_devices() -> Vec<InputDeviceInfo> {
    let drivers = DRIVERS.read();
    drivers
        .iter()
        .filter(|driver| driver.device_type() == DeviceType::Input)
        .enumerate()
        .map(|(idx, _driver)| InputDeviceInfo {
            name: format!("input{}", idx),
            id: idx as u32,
        })
        .collect()
}

/// 列出所有 RTC 设备
pub fn list_rtc_devices() -> Vec<RtcDeviceInfo> {
    let drivers = DRIVERS.read();
    drivers
        .iter()
        .filter(|driver| driver.device_type() == DeviceType::Rtc)
        .enumerate()
        .filter_map(|(idx, driver)| {
            Arc::clone(driver)
                .as_rtc_arc()
                .map(|rtc_dev| RtcDeviceInfo {
                    name: format!("rtc{}", idx),
                    id: idx as u32,
                    device: rtc_dev,
                })
        })
        .collect()
}
