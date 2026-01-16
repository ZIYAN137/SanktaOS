# 设备与驱动概览

设备子系统为内核提供“设备发现 + 驱动注册 + 中断派发 + 上层统一接口”的基础能力。

本目录只保留概览类文档；实现细节与 API 语义以对应源码文件的 rustdoc 注释为准。

## 源码导览（以 rustdoc 为准）

- OS 侧设备抽象层：`os/src/device/mod.rs`（重导出 `crates/device`，并提供平台/驱动实现）
- 设备树解析与探测：`os/src/device/device_tree.rs`
- VirtIO 传输层探测：`os/src/device/bus/virtio_mmio.rs`（以及 `os/src/device/bus/pcie.rs`）
- 中断控制器与派发：`os/src/device/irq/`（如 `plic.rs`）
- 设备接口抽象与全局注册表：`crates/device/src/`（`Driver` / `BlockDriver` / `NetDevice` / `Console` / `SerialDriver` / `RtcDriver` 等）

## 初始化顺序（概览）

以 RISC-V virt 平台为例，平台初始化通常依次完成：
1. `crate::device::init_device_ops()` 注册架构相关的中断使能回调
2. `virtio_mmio::driver_init()` / `plic::driver_init()` 等注册 DT compatible→探测函数
3. `device_tree::init()` 两轮遍历 DT：先初始化中断控制器，再初始化其他设备
4. 驱动自注册到全局表，并在需要时登记到中断管理器
