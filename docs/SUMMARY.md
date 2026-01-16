# 目录

[介绍](README.md)

# 内存管理

- [内存管理概述](mm/README.md)
  - [架构设计](mm/architecture.md)

# 日志系统

- [日志系统概述](log/README.md)
  - [架构设计](log/architecture.md)

# 网络

- [网络实现指南](net/network_implementation_guide.md)
  - [netperf / netserver 测试说明](net/netperf.md)

# 同步原语

- [同步机制概述](sync/README.md)
  - [设计要点](sync/design.md)

# 内核子系统

## 任务管理

- [任务管理概述](kernel/task/README.md)
  - [任务结构](kernel/task/task.md)
  - [调度器](kernel/task/scheduler.md)
  - [上下文切换](kernel/task/context.md)
  - [内存空间](kernel/task/memory_space.md)
  - [等待队列](kernel/task/wait_queue.md)

## 中断与陷阱

- [中断处理](kernel/trap/trap.md)
  - [上下文保存](kernel/trap/context.md)
  - [特权级切换](kernel/trap/switch.md)

# 虚拟文件系统

- [VFS 概述](vfs/README.md)
  - [整体架构](vfs/architecture.md)

# 文件系统实现

- [FS 模块概述](fs/README.md)

# 设备与驱动

- [设备与驱动概览](devices/README.md)

# 进程间通信
- [进程间通信概述](ipc/README.md)

# 系统调用

- [系统调用速查](syscall/README.md)

# 架构相关

## RISC-V

- [RISC-V寄存器](arch/riscv/riscv_register.md)
- [用户栈布局](arch/riscv/stack_layout.md)
- [多核启动](arch/riscv/smp_boot.md)
- [核间中断 (IPI)](arch/riscv/ipi.md)

## LoongArch64

- [LoongArch64](arch/loongarch/README.md)
  - [启动与用户态运行修复总结（comix-1 当前分支）](arch/loongarch/bringup_userland.md)


---

- [脚本工具](scripts/README.md)
  - [SimpleFS 镜像打包](scripts/make_init_simple_fs.md)
  - [文档链接转换](scripts/rewrite_links.md)
  - [代码质量检查](scripts/style-check.md)

---

[API 文档](api.md)
