# SanktaOS

![icon](icon.png)

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
- **SMP（多核）**：RISC-V 支持多核启动与 per-CPU 数据/调度（SBI 启动从核 + IPI reschedule）；LoongArch 目前以单核启动流程为主
- **评测适配**：根 `Makefile` 提供 `make all`（导出 `kernel-rv/kernel-la`，并生成 `disk*.img`）

### 内核与调度
- 任务模型：内核线程 + 用户进程（ELF 加载、用户栈/地址空间）
- 调度器：RR（Round-Robin）+ per-CPU 运行队列；跨核唤醒/迁移会触发 IPI reschedule
- 同步：自旋锁、per-CPU 数据（`PerCpu`）与抢占保护（`PreemptGuard`）


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
├── os/                      # 内核主 crate 与构建脚本
│   ├── src/
│   │   ├── arch/            # 架构相关代码（riscv/loongarch）
│   │   ├── kernel/          # 任务调度、系统调用、CPU 管理
│   │   ├── mm/              # 内存管理集成层
│   │   ├── vfs/             # VFS 集成层
│   │   ├── fs/              # 文件系统集成层
│   │   ├── device/          # 设备驱动集成层
│   │   ├── net/             # 网络栈集成层
│   │   ├── ipc/             # IPC 机制（pipe、signal、message、shm）
│   │   ├── sync/            # 同步原语集成层
│   │   └── main.rs          # 内核入口与初始化
│   ├── Makefile             # 构建、运行、测试、调试命令
│   └── rust-toolchain.toml  # Rust 工具链版本固定
├── crates/                  # 独立子系统 crate（可单独测试）
│   ├── mm/                  # 内存管理：物理页帧分配、虚拟地址空间、页表
│   ├── vfs/                 # 虚拟文件系统：路径解析、挂载点、FD 表、文件锁
│   ├── fs/                  # 文件系统实现：ext4、tmpfs、procfs、sysfs
│   ├── device/              # 设备驱动：VirtIO 框架、Block、Net、UART、RTC
│   ├── net/                 # 网络栈：TCP/IP 协议栈实现
│   ├── sync/                # 同步原语：自旋锁、per-CPU 数据、抢占保护
│   ├── klog/                # 内核日志：分级日志、格式化输出
│   └── uapi/                # 用户态 API：系统调用号、错误码、数据结构
├── user/                    # 用户态程序（自动编译并打包到 fs.img）
│   ├── src/                 # 用户程序源码（Rust）
│   └── c_src/               # C 语言用户程序
├── data/                    # 根文件系统基础内容
│   ├── busybox              # BusyBox 工具集
│   ├── init                 # 初始化脚本
│   └── ...                  # 其他基础文件
├── docs/                    # 设计文档（mdBook 格式）
│   ├── src/                 # 文档源码
│   └── book.toml            # mdBook 配置
├── scripts/                 # 构建工具脚本
│   ├── build_user.py        # 编译用户程序
│   ├── make_fs.py           # 生成 ext4 文件系统镜像
│   └── run_all_tests.py     # 运行全量测试
├── test-support/            # 测试支持文件
└── Makefile                 # 根 Makefile（评测入口：make all）
```

### 模块依赖关系

SanktaOS 采用严格的分层架构，依赖关系自上而下单向流动：

```
os/（内核主 crate）
 ├─→ crates/mm/      （内存管理）
 ├─→ crates/vfs/     （虚拟文件系统）
 ├─→ crates/fs/      （文件系统实现，依赖 vfs）
 ├─→ crates/device/  （设备驱动，依赖 mm）
 ├─→ crates/net/     （网络栈，依赖 device）
 ├─→ crates/sync/    （同步原语）
 ├─→ crates/klog/    （内核日志）
 └─→ crates/uapi/    （用户态 API 定义）
```

各 crate 可独立编译和测试（`cargo test -p <crate-name>`），确保模块化和可维护性。

## 环境依赖

- Rust nightly（工具链版本固定在 `os/rust-toolchain.toml`，目前为 `nightly-2025-10-28`）
- RISC-V 目标：`rustup target add riscv64gc-unknown-none-elf`
- LoongArch 目标：`rustup target add loongarch64-unknown-none`
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

## 本地开发（更详细）

### 构建与运行

```bash
# RISC-V（默认）
make build
make run

# LoongArch
make build ARCH=loongarch
make run ARCH=loongarch
```

### 调试（GDB）

```bash
# 终端 1：启动 QEMU，等待 gdb 连接
make debug

# 终端 2：连接 gdb
make gdb
```

LoongArch 调试同理（加上 `ARCH=loongarch`）。

### 测试与质量检查

```bash
# 运行内核单元测试（在 QEMU 中跑）
cd os && make test

# 快速风格检查（clippy + fmt --check）
cd os && make quick_check_style

# 跑全量测试（CI 使用）
python3 scripts/run_all_tests.py --stream
```

## 提交评测（OSCOMP / `make all`）

评测系统会在仓库根目录执行 `make all`。为满足 OS 大赛评测要求，本项目的根 `Makefile` 已提供标准入口，默认会：

- 构建并导出内核 ELF：
  - `kernel-rv`（RISC-V）
  - `kernel-la`（LoongArch）
- 生成（可选但推荐）运行时磁盘镜像：
  - `disk.img`
  - `disk-la.img`

在本地复现评测构建：

```bash
make all
```

可选参数（用于本地调试/防止卡死）：

- `OSCOMP_TEST_TIMEOUT_SECS=...`：为 oscomp runner 设置单个测试点超时（秒）；`0` 表示不启用
- `DISK_LINK=1`：本地快速路径，将 `disk*.img` 软链到 `os/fs-*.img`（不要用于提交/评测）

### 官方测试镜像（可选）

如果你想用官方测试镜像在本地跑 QEMU（更接近评测侧），可以把测试镜像放到 `test-images/`：

- `test-images/sdcard-rv.img`
- `test-images/sdcard-la.img`

然后运行：

```bash
make prepare-testimg
make run-oscomp-rv   # RISC-V
make run-oscomp-la   # LoongArch
```

运行参数（本地调试用）：

- RISC-V：`OSCOMP_RV_MEM=1G`、`OSCOMP_RV_SMP=1`
- LoongArch：`OSCOMP_LA_MEM=2G`、`OSCOMP_LA_SMP=1`

相关测试集参考：`testsuits-for-oskernel`（GitHub 分支 `pre-2025`）。

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

### 目录约定与大文件说明

- 评测侧要求根目录存在 `Makefile`，并通过 `make all` 完成构建；请不要删除根 `Makefile` 的 `all` 入口。
- 本仓库默认忽略 `.img` 等大文件与 `kernel-*`（本地构建产物）；评测/提交侧需要的产物由 `make all` 生成。

需要在 GitHub 仓库 Secrets 中配置：

- `GITLAB_REMOTE_URL`：GitLab 仓库 HTTP/HTTPS 地址（不包含用户名/密码）
- `GITLAB_USERNAME`
- `GITLAB_TOKEN`：具有 push 权限的 token（PAT / deploy token 均可）
- 可选：`GITLAB_BRANCH`：推送到 GitLab 的目标分支名（默认 `main`）

## 收尾检查清单（提交前）

- 本地确保能完整构建：`make all`
- 本地确保能跑基础测试：`cd os && make test`
- 确认 GitHub Actions 两个前置 workflow 在 `main` 的最新一次 `push` 上均为 success
- 确认 `Mirror to GitLab (vendored)` workflow 成功，并且 GitLab 仓库包含：
  - `os/vendor/`、`os/cargo-vendor-config.toml`（离线依赖）
  - `README.md` 已替换为 `README_gitlab.md` 的内容

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
