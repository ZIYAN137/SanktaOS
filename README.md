# SanktaOS

SanktaOS 是基于 [Comix](https://github.com/comix-kernel/comix) 进行深度重构和优化的 Rust 操作系统内核，聚焦 RISC-V 64 位和 LoongArch 架构。

## 项目定位

SanktaOS 在 Comix 的基础上进行了大刀阔斧的重构，目标是：
- **消除妥协**：移除 Comix 中的临时实现和技术债务
- **激进优化**：追求极致的性能和代码质量
- **完整兼容**：实现完整的 Linux ABI 兼容
- **双架构支持**：RISC-V 和 LoongArch 并重发展

## 核心特性

### 架构改进
- **模块化设计**：子系统拆分为独立 crate（mm、vfs、fs、device、net、sync、klog）
- **清晰分层**：严格的依赖层次，消除循环依赖
- **双架构支持**：RISC-V 64 和 LoongArch 64 平台

### 内存管理
- 物理页帧分配器、全局堆分配器
- SV39 地址空间管理
- ELF 加载与用户栈构建

### 文件系统
- 多层 VFS（路径解析、挂载点、FD 表、文件锁）
- Ext4 文件系统支持（基于 VirtIO-Block）
- tmpfs、procfs、sysfs 虚拟文件系统

### 设备驱动
- VirtIO MMIO 框架
- VirtIO-Block、VirtIO-Net
- UART console、RTC
- 设备树解析与驱动注册

### 系统调用与 IPC
- Linux ABI 子集实现
- pipe、message queue、shared memory、signal

### 网络栈
- 基础 TCP/IP 协议栈
- VirtIO-Net 网卡驱动

## 仓库结构

```
SanktaOS/
├── os/              # 内核主 crate 与构建脚本
├── crates/          # 独立子系统 crate
│   ├── mm/          # 内存管理
│   ├── vfs/         # 虚拟文件系统
│   ├── fs/          # 文件系统实现
│   ├── device/      # 设备驱动
│   ├── net/         # 网络栈
│   ├── sync/        # 同步原语
│   ├── klog/        # 内核日志
│   └── uapi/        # 用户态 API
├── data/            # 根文件系统基础内容（busybox、init 等）
├── docs/            # 设计文档（mdBook）
└── scripts/         # 构建工具脚本
```

## 环境依赖

- Rust nightly（rust-toolchain.toml 已固定版本）
- RISC-V 目标：`rustup target add riscv64gc-unknown-none-elf`
- LoongArch 目标：`rustup target add loongarch64-unknown-none-elf`
- QEMU：`qemu-system-riscv64` 或 `qemu-system-loongarch64`
- 构建工具：`make`、`python3`、`dd`、`mkfs.ext4`、`rust-objcopy`
- 可选：Docker/DevContainer

## 快速开始

```bash
# 构建内核（自动编译用户程序并生成 fs.img）
make build

# 在 QEMU 运行
cd os && make run

# 运行测试
cd os && make test

# 调试（两个终端）
cd os && make debug    # 终端 1：启动 QEMU 等待 GDB
cd os && make gdb      # 终端 2：连接 GDB

# 代码风格检查
cd os && make quick_check_style

# 格式化代码
make fmt
```

架构选择：`ARCH=riscv`（默认）或 `ARCH=loongarch`

## GitLab 提交流程（自动镜像）

本仓库包含一个 GitHub Actions 流程，会在 `main` 分支的前置 CI 全部成功后，将代码镜像推送到 GitLab（仅 HTTP），并为评测环境准备离线构建所需文件。

- 触发条件：`main` 分支 `push`，且以下 workflow 均成功：
  - `Run tests & Code Quality Checks`
  - `部署文档网站`
- 执行内容（仅在镜像分支/镜像提交中生效，不会改动 GitHub 的 `main` 历史）：
  - Rust 依赖离线化：运行 `cargo vendor` 生成 `os/vendor/`，并生成 `os/cargo-vendor-config.toml`
  - README 替换：若存在 `README_gitlab.md`，则覆盖 `README.md`（用于 GitLab/评测平台展示）
  - 推送方式：通过 HTTP Basic 认证 header 推送到 GitLab（不使用 SSH）
- 评测机隐藏目录过滤说明：
  - 评测机 clone 时会过滤掉隐藏目录（如 `.cargo`）。本项目的根 `Makefile` 在 `make all` 时会重建 `os/.cargo/config.toml`；
  - 若存在 `os/cargo-vendor-config.toml`（由镜像流程生成并提交到 GitLab），`make all` 会自动将其追加到 `.cargo/config.toml`，确保 Cargo 使用 vendored 依赖并离线构建。

需要在 GitHub 仓库 Secrets 中配置：

- `GITLAB_REMOTE_URL`：GitLab 仓库 HTTP/HTTPS 地址（不包含用户名/密码）
- `GITLAB_USERNAME`
- `GITLAB_TOKEN`：具有 push 权限的 token（PAT / deploy token 均可）
- 可选：`GITLAB_BRANCH`：推送到 GitLab 的目标分支名（默认 `main`）

## 文档

- 设计文档：[docs/README.md](docs/README.md)
- 贡献指南：[CONTRIBUTING.md](CONTRIBUTING.md)
- 开发指南：[CLAUDE.md](CLAUDE.md)

## 与 Comix 的关系

SanktaOS 是 Comix 的重构版本，继承了 Comix 的核心设计理念，但针对其局限性进行了大量改进。

### Comix 的局限性

Comix 作为教学/实验型内核，存在以下局限：
- **架构耦合**：子系统之间存在循环依赖，难以独立开发和测试
- **临时实现**：部分功能采用快速原型方案，缺乏生产级质量
- **性能妥协**：为了简化实现，牺牲了部分性能优化机会
- **架构支持不完整**：LoongArch 支持仅为脚手架，未完整实现
- **技术债务**：随着功能增加，积累了大量需要重构的代码

### SanktaOS 的改进

- **完全重构的模块化架构**：子系统拆分为独立 crate，消除循环依赖
- **更激进的性能优化**：追求极致性能，移除不必要的抽象层
- **更完善的 LoongArch 支持**：与 RISC-V 并重，完整实现双架构
- **消除技术债务**：大刀阔斧重构，建立清晰的代码规范

感谢 [Comix 项目](https://github.com/comix-kernel/comix) 提供的坚实基础。

## 贡献

欢迎提交 Issue 和 Pull Request！提交前请：
1. 阅读 [CONTRIBUTING.md](CONTRIBUTING.md)
2. 确保通过 `make fmt` 和 `cd os && make quick_check_style`
3. 运行测试 `cd os && make test`

## 许可证

本项目采用 [GPL-3.0](LICENSE) 许可证。

SanktaOS 基于 [Comix](https://github.com/comix-kernel/comix) 开发，Comix 同样采用 GPL-3.0 许可证。
