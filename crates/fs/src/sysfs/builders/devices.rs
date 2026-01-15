//! /sys/devices/ 设备层次结构构建器

use alloc::format;
use alloc::string::ToString;
use alloc::sync::Arc;

use device::block::BlockDriver;
use vfs::{FileMode, FsError, Inode};

use crate::sysfs::device_registry;
use crate::sysfs::inode::{SysfsAttr, SysfsInode};

/// 构建 /sys/devices/ 层次结构
pub fn build_platform_devices(root: &Arc<SysfsInode>) -> Result<(), FsError> {
    let devices_inode = root.lookup("devices")?;
    let devices_dir = devices_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    let platform_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));
    devices_dir.add_child("platform", platform_dir.clone())?;

    build_platform_block_devices(&platform_dir)?;
    build_platform_net_devices(&platform_dir)?;
    build_platform_tty_devices(&platform_dir)?;
    build_platform_input_devices(&platform_dir)?;
    build_platform_rtc_devices(&platform_dir)?;

    Ok(())
}

fn build_platform_block_devices(platform_dir: &Arc<SysfsInode>) -> Result<(), FsError> {
    for dev_info in device_registry::list_block_devices() {
        let dev_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

        let dev_attr = SysfsAttr {
            name: "dev".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let maj = dev_info.major;
                let min = dev_info.minor;
                Arc::new(move || Ok(format!("{}:{}\n", maj, min)))
            },
            store: None,
        };
        dev_dir.add_child("dev", SysfsInode::new_attribute(dev_attr))?;

        let uevent_attr = SysfsAttr {
            name: "uevent".to_string(),
            mode: FileMode::from_bits_truncate(0o644),
            show: {
                let maj = dev_info.major;
                let min = dev_info.minor;
                let name = dev_info.name.clone();
                Arc::new(move || {
                    Ok(format!(
                        "MAJOR={}\nMINOR={}\nDEVNAME={}\nDEVTYPE=disk\n",
                        maj, min, name
                    ))
                })
            },
            store: None,
        };
        dev_dir.add_child("uevent", SysfsInode::new_attribute(uevent_attr))?;

        let size_attr = SysfsAttr {
            name: "size".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let dev = dev_info.device.clone();
                Arc::new(move || {
                    let block_size = dev.block_size();
                    let total_blocks = dev.total_blocks();
                    let total_bytes = block_size * total_blocks;
                    let sectors = total_bytes / 512;
                    Ok(format!("{}\n", sectors))
                })
            },
            store: None,
        };
        dev_dir.add_child("size", SysfsInode::new_attribute(size_attr))?;

        let ro_attr = SysfsAttr {
            name: "ro".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: Arc::new(|| Ok("0\n".to_string())),
            store: None,
        };
        dev_dir.add_child("ro", SysfsInode::new_attribute(ro_attr))?;

        let removable_attr = SysfsAttr {
            name: "removable".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: Arc::new(|| Ok("0\n".to_string())),
            store: None,
        };
        dev_dir.add_child("removable", SysfsInode::new_attribute(removable_attr))?;

        let stat_attr = SysfsAttr {
            name: "stat".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: Arc::new(|| {
                Ok("       0        0        0        0        0        0        0        0        0        0        0\n".to_string())
            }),
            store: None,
        };
        dev_dir.add_child("stat", SysfsInode::new_attribute(stat_attr))?;

        build_queue_directory(&dev_dir, &dev_info.device)?;

        platform_dir.add_child(&dev_info.name, dev_dir)?;
    }

    Ok(())
}

fn build_queue_directory(
    dev_dir: &Arc<SysfsInode>,
    device: &Arc<dyn BlockDriver>,
) -> Result<(), FsError> {
    let queue_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

    let logical_block_size_attr = SysfsAttr {
        name: "logical_block_size".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: {
            let dev = device.clone();
            Arc::new(move || Ok(format!("{}\n", dev.block_size())))
        },
        store: None,
    };
    queue_dir.add_child(
        "logical_block_size",
        SysfsInode::new_attribute(logical_block_size_attr),
    )?;

    let physical_block_size_attr = SysfsAttr {
        name: "physical_block_size".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: {
            let dev = device.clone();
            Arc::new(move || Ok(format!("{}\n", dev.block_size())))
        },
        store: None,
    };
    queue_dir.add_child(
        "physical_block_size",
        SysfsInode::new_attribute(physical_block_size_attr),
    )?;

    let hw_sector_size_attr = SysfsAttr {
        name: "hw_sector_size".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: Arc::new(|| Ok("512\n".to_string())),
        store: None,
    };
    queue_dir.add_child(
        "hw_sector_size",
        SysfsInode::new_attribute(hw_sector_size_attr),
    )?;

    let max_sectors_kb_attr = SysfsAttr {
        name: "max_sectors_kb".to_string(),
        mode: FileMode::from_bits_truncate(0o644),
        show: Arc::new(|| Ok("1280\n".to_string())),
        store: None,
    };
    queue_dir.add_child(
        "max_sectors_kb",
        SysfsInode::new_attribute(max_sectors_kb_attr),
    )?;

    let rotational_attr = SysfsAttr {
        name: "rotational".to_string(),
        mode: FileMode::from_bits_truncate(0o644),
        show: Arc::new(|| Ok("1\n".to_string())),
        store: None,
    };
    queue_dir.add_child("rotational", SysfsInode::new_attribute(rotational_attr))?;

    dev_dir.add_child("queue", queue_dir)?;
    Ok(())
}

fn build_platform_net_devices(platform_dir: &Arc<SysfsInode>) -> Result<(), FsError> {
    for dev_info in device_registry::list_net_devices() {
        let dev_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

        let uevent_attr = SysfsAttr {
            name: "uevent".to_string(),
            mode: FileMode::from_bits_truncate(0o644),
            show: {
                let name = dev_info.name.clone();
                let ifindex = dev_info.ifindex;
                Arc::new(move || Ok(format!("INTERFACE={}\nIFINDEX={}\n", name, ifindex)))
            },
            store: None,
        };
        dev_dir.add_child("uevent", SysfsInode::new_attribute(uevent_attr))?;

        let address_attr = SysfsAttr {
            name: "address".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let dev = dev_info.device.clone();
                Arc::new(move || {
                    let mac = dev.mac_address();
                    Ok(format!(
                        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}\n",
                        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
                    ))
                })
            },
            store: None,
        };
        dev_dir.add_child("address", SysfsInode::new_attribute(address_attr))?;

        let mtu_attr = SysfsAttr {
            name: "mtu".to_string(),
            mode: FileMode::from_bits_truncate(0o644),
            show: {
                let dev = dev_info.device.clone();
                Arc::new(move || Ok(format!("{}\n", dev.mtu())))
            },
            store: None,
        };
        dev_dir.add_child("mtu", SysfsInode::new_attribute(mtu_attr))?;

        let operstate_attr = SysfsAttr {
            name: "operstate".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: Arc::new(|| Ok("up\n".to_string())),
            store: None,
        };
        dev_dir.add_child("operstate", SysfsInode::new_attribute(operstate_attr))?;

        let carrier_attr = SysfsAttr {
            name: "carrier".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: Arc::new(|| Ok("1\n".to_string())),
            store: None,
        };
        dev_dir.add_child("carrier", SysfsInode::new_attribute(carrier_attr))?;

        let ifindex_attr = SysfsAttr {
            name: "ifindex".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let ifindex = dev_info.ifindex;
                Arc::new(move || Ok(format!("{}\n", ifindex)))
            },
            store: None,
        };
        dev_dir.add_child("ifindex", SysfsInode::new_attribute(ifindex_attr))?;

        let type_attr = SysfsAttr {
            name: "type".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: Arc::new(|| Ok("1\n".to_string())),
            store: None,
        };
        dev_dir.add_child("type", SysfsInode::new_attribute(type_attr))?;

        platform_dir.add_child(&dev_info.name, dev_dir)?;
    }

    Ok(())
}

fn build_platform_tty_devices(platform_dir: &Arc<SysfsInode>) -> Result<(), FsError> {
    for dev_info in device_registry::list_tty_devices() {
        let dev_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

        let dev_attr = SysfsAttr {
            name: "dev".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let maj = dev_info.major;
                let min = dev_info.minor;
                Arc::new(move || Ok(format!("{}:{}\n", maj, min)))
            },
            store: None,
        };
        dev_dir.add_child("dev", SysfsInode::new_attribute(dev_attr))?;

        let uevent_attr = SysfsAttr {
            name: "uevent".to_string(),
            mode: FileMode::from_bits_truncate(0o644),
            show: {
                let maj = dev_info.major;
                let min = dev_info.minor;
                let name = dev_info.name.clone();
                Arc::new(move || Ok(format!("MAJOR={}\nMINOR={}\nDEVNAME={}\n", maj, min, name)))
            },
            store: None,
        };
        dev_dir.add_child("uevent", SysfsInode::new_attribute(uevent_attr))?;

        platform_dir.add_child(&dev_info.name, dev_dir)?;
    }

    Ok(())
}

fn build_platform_input_devices(platform_dir: &Arc<SysfsInode>) -> Result<(), FsError> {
    for dev_info in device_registry::list_input_devices() {
        let dev_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

        let uevent_attr = SysfsAttr {
            name: "uevent".to_string(),
            mode: FileMode::from_bits_truncate(0o644),
            show: {
                let name = dev_info.name.clone();
                Arc::new(move || Ok(format!("NAME={}\n", name)))
            },
            store: None,
        };
        dev_dir.add_child("uevent", SysfsInode::new_attribute(uevent_attr))?;

        let name_attr = SysfsAttr {
            name: "name".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let name = dev_info.name.clone();
                Arc::new(move || Ok(format!("{}\n", name)))
            },
            store: None,
        };
        dev_dir.add_child("name", SysfsInode::new_attribute(name_attr))?;

        platform_dir.add_child(&dev_info.name, dev_dir)?;
    }

    Ok(())
}

fn build_platform_rtc_devices(platform_dir: &Arc<SysfsInode>) -> Result<(), FsError> {
    for dev_info in device_registry::list_rtc_devices() {
        let dev_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

        let uevent_attr = SysfsAttr {
            name: "uevent".to_string(),
            mode: FileMode::from_bits_truncate(0o644),
            show: {
                let name = dev_info.name.clone();
                Arc::new(move || Ok(format!("RTC_NAME={}\n", name)))
            },
            store: None,
        };
        dev_dir.add_child("uevent", SysfsInode::new_attribute(uevent_attr))?;

        let name_attr = SysfsAttr {
            name: "name".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let name = dev_info.name.clone();
                Arc::new(move || Ok(format!("{}\n", name)))
            },
            store: None,
        };
        dev_dir.add_child("name", SysfsInode::new_attribute(name_attr))?;

        let date_attr = SysfsAttr {
            name: "date".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let rtc = dev_info.device.clone();
                Arc::new(move || {
                    let dt = rtc.read_datetime();
                    Ok(format!("{:04}-{:02}-{:02}\n", dt.year, dt.month, dt.day))
                })
            },
            store: None,
        };
        dev_dir.add_child("date", SysfsInode::new_attribute(date_attr))?;

        let time_attr = SysfsAttr {
            name: "time".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let rtc = dev_info.device.clone();
                Arc::new(move || {
                    let dt = rtc.read_datetime();
                    Ok(format!(
                        "{:02}:{:02}:{:02}\n",
                        dt.hour, dt.minute, dt.second
                    ))
                })
            },
            store: None,
        };
        dev_dir.add_child("time", SysfsInode::new_attribute(time_attr))?;

        platform_dir.add_child(&dev_info.name, dev_dir)?;
    }

    Ok(())
}
