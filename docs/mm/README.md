# MM 子系统文档

## 简介

MM（Memory Management，内存管理）子系统是 SanktaOS 内核的核心组件，负责管理系统的物理和虚拟内存。该子系统采用清晰的分层架构，将架构无关的抽象层与架构特定的实现层分离，支持 RISC-V 和 LoongArch 等多种硬件平台。

### 主要功能

- **物理内存管理**：物理帧的分配、回收和跟踪
- **虚拟内存管理**：地址空间管理、页表操作、内存映射
- **内核堆分配**：支持动态内存分配的全局分配器
- **架构抽象**：统一的接口支持多种硬件架构

### 可选特性（feature flags，默认关闭）

SanktaOS 的部分内存管理能力通过 feature flag 控制（默认关闭，避免引入额外复杂度/回归风险）：

- `demand_paging`：对 `MapType::Lazy` 区域启用按需分配（首次访问触发缺页分配/建立映射）
- `cow`：fork 采用 Copy-on-Write，共享已物化页面并对“私有可写”页做写保护，写缺页时复制
- `thp`：透明大页（THP），目前支持 RISC-V SV39 的 2MiB（512*4KiB）leaf mapping，以及 split/collapse

启用方式示例（RISC-V）：

```bash
cd os
cargo build --target riscv64gc-unknown-none-elf --features demand_paging,cow,thp
```

## 模块结构（代码位置）

```
crates/mm/src/                      # 架构无关的内存管理层（mm crate）
│
├── address/ ...................... 地址抽象层
│   ├── address.rs ............... 物理/虚拟地址类型 (Paddr/Vaddr)
│   ├── page_num.rs .............. 物理/虚拟页号类型 (Ppn/Vpn)
│   └── operations.rs ............ 地址运算 trait 定义
│
├── frame_allocator.rs ............ 物理帧分配器（RAII + 连续/对齐分配）
│
├── page_table/ ................... 页表抽象层
│   ├── page_table.rs ............ PageTableInner trait 定义
│   └── page_table_entry.rs ...... PTE trait 及通用标志位
│
└── memory_space/ ................. 地址空间管理
    ├── memory_space.rs .......... MemorySpace 结构及空间创建
    └── mapping_area.rs .......... MappingArea 映射区域管理

os/src/mm/                          # os crate 中的 mm 相关实现（os-specific）
│
├── mod.rs ........................ MM 初始化入口（组装 crates/mm 提供的基础能力）
├── global_allocator.rs ........... 内核堆分配器（talc）
└── memory_space.rs ............... 进程/内核地址空间创建与管理（os-specific）

os/src/arch/{riscv,loongarch}/mm/   # 架构特定实现层
│
├── mod.rs ........................ 地址转换函数
├── page_table.rs ................. PageTableInner 实现
└── page_table_entry.rs ........... PageTableEntry 实现
```

## 文档导航

### 核心概念

- **[整体架构](architecture.md)** - MM 子系统的分层设计、模块依赖关系和初始化流程

### API 与实现细节

从阶段 2 开始，MM 的 API 参考与实现细节以“代码注释（rustdoc）”为主：

- `crates/mm/src/`：mm crate 的公共 API 与核心实现说明（推荐通过 `cargo doc` 查看）
- `os/src/mm/`：与内核启动/进程管理强相关的地址空间创建、全局分配器等实现

## 设计原则

### 1. 架构抽象

MM 子系统使用 trait 系统实现架构抽象，架构特定代码必须实现以下接口：

- `vaddr_to_paddr()` / `paddr_to_vaddr()` - 地址转换函数
- `PageTableInner` trait - 页表操作接口
- `PageTableEntry` trait - 页表项操作接口

### 2. 安全性保障

- **RAII 模式**：物理帧通过 `FrameTracker` 自动管理生命周期
- **类型安全**：物理地址和虚拟地址使用不同类型，防止混用
- **所有权系统**：利用 Rust 的所有权机制防止内存泄漏

### 3. 性能优化

- **位图分配**：使用位图跟踪帧状态，支持 O(1) 释放和快速查找
- **局部性优化**：last_alloc_hint 缓存上次分配位置，减少扫描开销
- **直接映射**：内核空间使用直接映射，避免页表查找开销
- **对齐分配**：支持对齐的连续帧分配，优化 DMA 等场景

## 重要约定

### Range 语义

所有 range 类型均遵循 **左闭右开区间** 语义：

- `AddressRange::new(start, end)` 表示区间 **[start, end)**
- `PageNumRange::new(start, end)` 表示区间 **[start, end)**
- 迭代器遍历时包含 `start`，不包含 `end`

### 内存布局

SanktaOS 源自 Comix，并沿用其**高半核（Higher Half Kernel）**设计，虚拟地址空间分为两个主要区域：

```
虚拟地址空间布局（从高地址到低地址）:

═══════════════════════════════════════════════════════════════════
                        高半核（内核空间）
═══════════════════════════════════════════════════════════════════
0xFFFF_FFFF_FFFF_FFFF
        |
        ... (向高地址扩展的物理内存直接映射区)
        |
[可用物理内存帧]          ← [ekernel, MEMORY_END) 直接映射
[内核堆 Heap]             ← sheap ~ eheap (16MB，talc 分配器)
[内核 BSS 段 .bss]        ← sbss ~ ebss
[内核数据段 .data]        ← sdata ~ edata
[内核只读数据段 .rodata]  ← srodata ~ erodata
[内核代码段 .text]        ← stext ~ etext
        |
0xFFFF_FFC0_8020_0000 ← VIRTUAL_BASE (内核加载地址)
        |
地址上半底部 ← VADDR_START (内核空间基址)

物理地址映射: vaddr = paddr | 0xFFFF_FFC0_0000_0000

═══════════════════════════════════════════════════════════════════
                         低半核（用户空间）
═══════════════════════════════════════════════════════════════════
地址下半顶部
        |
[USER_STACK]              ← 用户栈区域 (4MB)
        |
        ... (动态扩展空间)
        |
[USER_HEAP]               ← 用户堆区域（可动态扩展，最大 64MB）
[USER_DATA]               ← 用户数据段 (.data, .bss)
[USER_TEXT]               ← 用户代码段 (.text)
        |
0x0000_0000_0000_0000
```

**关键地址常量**：

*内核空间*：
- `VADDR_START = 0xFFFF_FFC0_0000_0000` - 内核虚拟地址空间基址（RISC-V）
- `VIRTUAL_BASE = 0xFFFF_FFC0_8020_0000` - 内核实际加载地址
- `PHYSICAL_BASE = 0x8020_0000` - 内核物理加载地址
- `MEMORY_END = 0x8800_0000` - 物理内存结束地址（128MB，QEMU virt）

*用户空间*：
- `USER_STACK_TOP - 用户栈顶

**地址转换规则**（RISC-V SV39）：
- 物理地址 → 虚拟地址：`vaddr = paddr | VADDR_START`
- 虚拟地址 → 物理地址：`paddr = vaddr & 0x0000_003F_FFFF_FFFF`

## 快速开始

### 初始化流程

MM 子系统在 `mm::init()` 中按以下顺序初始化：

1. **内核堆分配器初始化** - 初始化全局堆分配器（使用链接器脚本定义的静态堆区域）
2. **物理帧分配器初始化** - 管理 `[ekernel, MEMORY_END)` 物理内存区域（需要堆分配来创建位图）
3. **内核地址空间创建** - 创建并激活内核页表

```rust
// os/src/mm/mod.rs:48
pub fn init() {
    // 1. 初始化堆分配器
    init_heap();

    // 2. 初始化物理帧分配器
    let ekernel_paddr = unsafe { vaddr_to_paddr(ekernel as usize) };
    init_frame_allocator(Ppn::from_addr_ceil(ekernel_paddr),
                         Ppn::from_addr_floor(MEMORY_END));

    // 3. 创建并激活内核地址空间
    #[cfg(target_arch = "riscv64")] {
        let root_ppn = with_kernel_space(|space| space.root_ppn());
        crate::arch::mm::PageTableInner::activate(root_ppn);
    }
}
```

### 常见操作示例

#### 分配物理帧

```rust
use crate::mm::frame_allocator::alloc_frame;

// 分配单个物理帧（自动释放）
let frame = alloc_frame().expect("Failed to allocate frame");
let ppn = frame.ppn();
```

#### 创建地址映射

```rust
use crate::mm::memory_space::MemorySpace;

// 创建用户地址空间
let mut space = MemorySpace::from_elf(elf_data);

// 映射匿名内存
space.mmap(start_vaddr, len, prot);
```

#### 地址转换

```rust
use crate::mm::address::{Vaddr, Paddr};

// 虚拟地址转物理地址
let vaddr = Vaddr::new(0xffff_ffc0_8000_0000);
let paddr = vaddr.to_paddr();

// 物理地址转虚拟地址
let vaddr_back = paddr.to_vaddr();
```

## 相关资源

- **源代码位置**：`os/src/mm/` 和 `os/src/arch/{riscv,loongarch}/mm/`
- **配置常量**：`os/src/config.rs`

## 版本信息

- **Rust 版本**：nightly-2025-01-13
- **支持架构**：RISC-V (SV39), LoongArch (TODO)
- **页面大小**：4KB（基础页）；可选支持 2MiB（`--features thp`，RISC-V SV39）

## Demand Paging / COW / THP 速览

本节给出三个特性的关键语义约定（详细流程见 [整体架构](architecture.md)）。

### Demand Paging（`demand_paging`）

- Lazy VMA：`MapType::Lazy` 仅占位虚拟地址范围，不预先建立 PTE；首次访问触发缺页后分配物理页并映射。
- 权限：缺页处理会基于 VMA 元数据（READ/WRITE/EXEC）判定访问是否合法；非法访问返回 `SIGSEGV`。

### Copy-on-Write（`cow`）

- fork：父子共享已物化页帧；对“私有可写”（非 `MAP_SHARED` 且 VMA permission 包含 WRITE）页清除 PTE 的 WRITE 位。
- 写缺页：若共享计数>1，则分配新页、复制旧页内容并重映射；否则仅恢复 WRITE 权限（无需复制）。
- 内核写用户内存：`copy_to_user` 路径也会触发 COW（通过“翻译失败/不可写 → 缺页处理 → 重试”实现）。

### THP（`thp`，RISC-V SV39 2MiB）

- 页大小：2MiB = 512 * 4KiB，使用 SV39 level-1 leaf PTE 表示。
- collapse（策略 A，机会式）：对 **Lazy 匿名映射**（无文件映射）在 fault-in 后尝试 collapse：
  - 阈值：当一个 2MiB chunk 内 materialize 的 4KiB 页数达到 **400/512** 时触发尝试；
  - 物理内存：需要分配 **2MiB 对齐的连续页帧**（512 页）作为新 backing，复制已存在的 4KiB 页内容；
  - 跟踪：collapse 后使用 `TrackedFrames::Contiguous` 跟踪整个 2MiB chunk。
- split：在需要对大页做“部分操作”（如 munmap/mprotect/写缺页）前先 split 为 512 个 4KiB leaf。
- 与 COW 的交互：写缺页策略为 **split -> COW**（先拆大页，再按 4KiB 粒度处理 COW）。

### SMP / TLB shootdown 约束

页表批量修改（map/unmap/update_flags/split/collapse）需要与多核 TLB 一致性协同：

- 批处理：通过 `TlbBatchContextWrapper` 聚合刷新，减少 IPI 次数；
- 同步：在可能释放/回收页帧的路径上使用“同步 shootdown”（等待其他 CPU ack）以避免远端 CPU 使用旧 TLB 访问已释放物理页。
