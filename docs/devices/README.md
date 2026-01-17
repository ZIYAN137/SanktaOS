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

### 设备树的早期扫描与系统依赖

**关键系统信息的获取：**

在设备驱动初始化之前，`device_tree::init()` 会调用 `early_init()` 从设备树中提取关键的系统配置信息：
- **CPU 数量**（`NUM_CPU`）：从设备树的 `/cpus` 节点解析，供 SMP 多核启动使用
- **时钟频率**（`CLOCK_FREQ`）：从 CPU 节点的 `timebase-frequency` 或 `clock-frequency` 属性读取，供定时器初始化使用
- **内存区域信息**：从设备树的 `/memory` 节点读取，用于验证和日志输出（实际内存管理使用配置中的 `MEMORY_END`）

这些信息在 `device_tree::init()` 的第一时间被解析（第 97 行调用 `early_init()`），确保后续的 SMP 启动（`boot_secondary_cpus()`）和定时器初始化（`timer::init()`）能够获取正确的系统参数。

### 两阶段初始化的设计原理

**为什么需要两阶段初始化？**

大多数设备驱动在初始化时需要注册中断处理函数到中断控制器（如 PLIC），这要求中断控制器必须先于其他设备完成初始化。两阶段初始化通过设备树的 `interrupt-controller` 属性区分中断控制器与普通设备，确保正确的初始化顺序。

**实现机制：**

核心实现位于 `os/src/device/device_tree.rs` 的 `init()` 和 `walk_dt()` 函数：
- `device_tree::init()` 调用两次 `walk_dt()`：第一次传入 `intc_only=true`，第二次传入 `intc_only=false`
- `walk_dt()` 通过检查设备树节点的 `interrupt-controller` 属性，决定是否在当前阶段初始化该设备
- 第一阶段：仅初始化具有 `interrupt-controller` 属性的设备（如 PLIC）
- 第二阶段：初始化其他所有设备（UART、VirtIO、RTC 等）

### 详细的初始化流程

完整的调用链（以 RISC-V virt 平台为例）：

```
os/src/arch/riscv/boot/mod.rs::main()
  ├─ crate::device::init_device_ops()          // 注册架构相关回调
  └─ platform::init()                          // os/src/arch/riscv/platform/virt.rs
       ├─ serial::uart16550::driver_init()     // 注册 "ns16550a" → 探测函数
       ├─ bus::virtio_mmio::driver_init()      // 注册 "virtio,mmio" → 探测函数
       ├─ irq::plic::driver_init()             // 注册 "riscv,plic0" → 探测函数
       ├─ rtc::rtc_goldfish::driver_init()     // 注册 "google,goldfish-rtc" → 探测函数
       └─ device_tree::init()                  // os/src/device/device_tree.rs
            ├─ walk_dt(fdt, intc_only=true)    // 第一阶段：初始化中断控制器
            │    └─ 调用 plic::init_dt()       // 创建 PLIC 驱动，注册到 IRQ_MANAGER
            └─ walk_dt(fdt, intc_only=false)   // 第二阶段：初始化其他设备
                 ├─ 调用 uart16550::init()     // 创建串口驱动
                 ├─ 调用 virtio_mmio::virtio_probe()  // 探测 VirtIO 设备
                 └─ 调用 rtc_goldfish::init()  // 创建 RTC 驱动
```

**关键要点：**
- 驱动的 `driver_init()` 函数只是注册 compatible 字符串和探测函数到 `DEVICE_TREE_REGISTRY`，不执行实际初始化
- 实际初始化发生在 `device_tree::init()` 遍历设备树时，根据 compatible 匹配调用对应的探测函数
- 中断控制器驱动（如 PLIC）在第一阶段完成初始化并注册到 `IRQ_MANAGER`，为第二阶段的设备提供中断注册服务

## 驱动注册机制

### 注册表与全局数据结构

设备子系统使用以下全局注册表管理驱动：

- **`DEVICE_TREE_REGISTRY`**（`os/src/device/device_tree.rs`）：DT compatible 字符串到探测函数的映射表，驱动通过 `driver_init()` 注册到此表
- **`DEVICE_TREE_INTC`**（`os/src/device/device_tree.rs`）：设备树 phandle 到中断控制器驱动的映射表，用于设备查找其父中断控制器
- **`DRIVERS`**（`crates/device/src/lib.rs`）：所有驱动的全局列表
- **`IRQ_MANAGER`**（`crates/device/src/lib.rs`）：全局中断管理器，负责中断派发
- **类型化驱动列表**：`BLK_DRIVERS`、`NETWORK_DEVICES`、`SERIAL_DRIVERS`、`RTC_DRIVERS` 等，按设备类型分类

### 驱动开发流程

实现一个新驱动通常需要以下步骤：

1. **实现 `driver_init()` 函数**：在平台初始化时被调用，将 DT compatible 字符串和探测函数注册到 `DEVICE_TREE_REGISTRY`
2. **实现探测函数**：解析设备树节点（MMIO 地址、中断号等），创建驱动实例
3. **注册到全局表**：将驱动实例添加到 `DRIVERS` 和对应的类型化列表（如 `BLK_DRIVERS`）
4. **注册中断（如需要）**：调用 `IRQ_MANAGER.register_irq()` 注册中断处理函数
5. **实现设备接口 trait**：根据设备类型实现 `BlockDriver`、`NetDevice`、`SerialDriver` 等 trait

### 代码示例指引

- **简单驱动示例**：`os/src/device/serial/uart16550.rs` - 展示基本的 MMIO 设备初始化和驱动注册
- **中断控制器示例**：`os/src/device/irq/plic.rs` - 展示如何实现中断控制器并注册到 `IRQ_MANAGER` 和 `DEVICE_TREE_INTC`
- **总线驱动示例**：`os/src/device/bus/virtio_mmio.rs` - 展示如何实现总线探测并根据设备类型分发到具体驱动
- **块设备驱动示例**：`os/src/device/block/virtio_blk.rs` - 展示如何实现 `BlockDriver` trait 并注册到 `BLK_DRIVERS`

详细的 API 语义与实现细节请参考对应源码文件的 rustdoc 注释。
