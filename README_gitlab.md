# SanktaOS (GitLab 提交版说明)

本 README 面向 OS 大赛评测环境（GitLab 仓库展示 / 评测机构建约束），与 GitHub 上的开发版说明有所不同。

## 上游仓库（主要开发在 GitHub）

本项目主要在 GitHub 上开发与维护：

- 上游仓库：`https://github.com/ZIYAN137/SanktaOS/`

此 GitLab 仓库通常由 CI 自动镜像（包含 Rust vendor 等评测所需内容），用于提交与评测。

## 评测构建要求（必须）

- 评测机会在项目根目录执行：`make all`
- `make all` 必须生成以下文件（ELF 内核）：
  - `kernel-rv`：RISC-V 内核
  - `kernel-la`：LoongArch 内核
- 若系统需要额外镜像文件，可生成（可选，但本项目会生成）：
  - `disk.img`
  - `disk-la.img`

本仓库根目录 `Makefile` 的 `all` 目标已按上述要求配置。

## 离线依赖 / 隐藏目录过滤（重要）

评测系统在 clone 时会过滤所有隐藏文件和目录（例如 `.cargo/`）。

为避免构建依赖隐藏目录，本仓库做了两件事：

1. `make all` 会在构建时重建 `os/.cargo/config.toml`（即便 `.cargo/` 在评测机上被过滤也能恢复）。
2. 为避免在线下载依赖，GitLab 仓库中会包含 Rust vendored 依赖：
   - `os/vendor/`
   - `os/cargo-vendor-config.toml`

当 `os/cargo-vendor-config.toml` 存在时，`make all` 会自动将其追加到 `os/.cargo/config.toml`，使 Cargo 使用 vendored 源并以离线模式构建。

## 运行说明（评测侧）

评测侧会使用 QEMU 虚拟机启动内核，并挂载包含测试点的 EXT4 镜像（无分区表）。系统启动后需要扫描磁盘根目录中的 `xxxxx_testcode.sh` 等脚本并依次执行，按要求输出测试提示信息；执行完后应主动关机。

本项目会在构建时生成 `disk.img` / `disk-la.img`（用于系统自带的运行时文件），评测启动 QEMU 时可能会同时挂载评测机的测试镜像与本项目的 `disk*.img`。

## 演示与讲解视频
通过网盘分享的文件：
链接: https://pan.baidu.com/s/1RkagFa-qSt-CkAyzYlVFqA?pwd=jtd4 提取码: jtd4 复制这段内容后打开百度网盘手机App，操作更方便哦