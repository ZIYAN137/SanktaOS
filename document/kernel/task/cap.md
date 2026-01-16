# 能力管理（Capabilities）

## 概述

能力（Capabilities）是 Linux 内核提供的一种细粒度权限控制机制，将传统的 root 超级用户权限分解为多个独立的能力位。每个能力代表执行特定特权操作的权限，进程可以拥有部分能力而不需要完整的 root 权限。

SanktaOS 实现了与 Linux 兼容的能力系统，支持 41 种能力和 5 个能力集。

**代码位置**：`os/src/kernel/task/cap.rs`

## 核心概念

### 能力系统的意义

传统 Unix 系统只有两种用户：
- **root（UID=0）**：拥有所有权限
- **普通用户**：权限受限

这种模型存在问题：
- **权限过大**：root 可以做任何事，安全风险高
- **权限不足**：某些程序需要部分特权，但不得不以 root 运行

能力系统解决了这个问题：
- 将 root 权限分解为 41 个独立的能力
- 进程可以只拥有需要的能力，而不是全部权限
- 降低了安全风险，实现了最小权限原则

### 五个能力集

Linux 为每个进程维护 5 个能力集：

1. **Effective（有效能力集）**：
   - 当前实际生效的能力
   - 内核进行权限检查时使用这个集合
   - 可以动态启用/禁用

2. **Permitted（允许能力集）**：
   - 进程可以使用的能力上限
   - 可以将 permitted 中的能力添加到 effective
   - 不能超出 permitted 的范围

3. **Inheritable（可继承能力集）**：
   - execve() 时可以继承给新程序的能力
   - 与文件能力配合使用

4. **Bounding（边界能力集）**：
   - 能力的绝对上限
   - 限制进程可以获得的能力
   - 只能减少，不能增加

5. **Ambient（环境能力集）**：
   - 保持跨 execve() 的能力
   - 简化非特权程序获取能力的流程

## 核心数据结构

### Capabilities

能力位标志，使用 bitflags 实现。

```rust
pub struct Capabilities: u64 {
    const CHOWN              = 1 << 0;
    const DAC_OVERRIDE       = 1 << 1;
    // ... 共 41 个能力
}
```

**完整能力列表**：

| 能力 | 值 | 说明 |
|------|-----|------|
| `CHOWN` | 0 | 改变文件所有者 |
| `DAC_OVERRIDE` | 1 | 绕过文件读写执行权限检查 |
| `DAC_READ_SEARCH` | 2 | 绕过文件读权限检查 |
| `FOWNER` | 3 | 绕过文件所有者检查 |
| `FSETID` | 4 | 不清除 setuid/setgid 位 |
| `KILL` | 5 | 发送信号给任意进程 |
| `SETGID` | 6 | 修改进程 GID |
| `SETUID` | 7 | 修改进程 UID |
| `SETPCAP` | 8 | 传递能力 |
| `LINUX_IMMUTABLE` | 9 | 设置不可变标志 |
| `NET_BIND_SERVICE` | 10 | 绑定特权端口 (<1024) |
| `NET_BROADCAST` | 11 | 网络广播 |
| `NET_ADMIN` | 12 | 网络管理操作 |
| `NET_RAW` | 13 | 使用 RAW 和 PACKET socket |
| `IPC_LOCK` | 14 | 锁定内存 |
| `IPC_OWNER` | 15 | 绕过 IPC 所有权检查 |
| `SYS_MODULE` | 16 | 加载/卸载内核模块 |
| `SYS_RAWIO` | 17 | 执行 I/O 端口操作 |
| `SYS_CHROOT` | 18 | 使用 chroot() |
| `SYS_PTRACE` | 19 | 使用 ptrace() |
| `SYS_PACCT` | 20 | 进程统计 |
| `SYS_ADMIN` | 21 | 系统管理操作 |
| `SYS_BOOT` | 22 | 重启系统 |
| `SYS_NICE` | 23 | 修改进程优先级 |
| `SYS_RESOURCE` | 24 | 覆盖资源限制 |
| `SYS_TIME` | 25 | 设置系统时间 |
| `SYS_TTY_CONFIG` | 26 | 配置 TTY |
| `MKNOD` | 27 | 创建设备文件 |
| `LEASE` | 28 | 建立文件租约 |
| `AUDIT_WRITE` | 29 | 写入审计日志 |
| `AUDIT_CONTROL` | 30 | 配置审计 |
| `SETFCAP` | 31 | 设置文件能力 |
| `MAC_OVERRIDE` | 32 | 覆盖 MAC 策略 |
| `MAC_ADMIN` | 33 | 配置 MAC |
| `SYSLOG` | 34 | 访问内核日志 |
| `WAKE_ALARM` | 35 | 触发唤醒告警 |
| `BLOCK_SUSPEND` | 36 | 阻止系统挂起 |
| `AUDIT_READ` | 37 | 读取审计日志 |
| `PERFMON` | 38 | 性能监控 |
| `BPF` | 39 | BPF 操作 |
| `CHECKPOINT_RESTORE` | 40 | 检查点/恢复 |

---

### CapabilitySet

能力集合，包含 5 个能力集。

```rust
pub struct CapabilitySet {
    pub effective: Capabilities,
    pub permitted: Capabilities,
    pub inheritable: Capabilities,
    pub bounding: Capabilities,
    pub ambient: Capabilities,
}
```

## API 参考

### Capabilities API

#### full()

```rust
pub const fn full() -> Self
```

创建拥有所有能力的能力集（root 用户）。

**返回值**：包含所有 41 个能力的集合

**示例**：
```rust
let root_caps = Capabilities::full();
assert!(root_caps.contains(Capabilities::CHOWN));
assert!(root_caps.contains(Capabilities::SYS_ADMIN));
```

---

#### empty_set()

```rust
pub const fn empty_set() -> Self
```

创建空能力集。

**返回值**：不包含任何能力的集合

**示例**：
```rust
let no_caps = Capabilities::empty_set();
assert!(no_caps.is_empty());
```

---

### CapabilitySet API

#### full()

```rust
pub const fn full() -> Self
```

创建拥有所有能力的能力集合（root 用户）。

**返回值**：所有 5 个能力集都包含全部能力

**示例**：
```rust
let root_capset = CapabilitySet::full();
assert!(root_capset.has(Capabilities::SYS_ADMIN));
```

---

#### empty()

```rust
pub const fn empty() -> Self
```

创建空能力集合。

**返回值**：所有 5 个能力集都为空

**示例**：
```rust
let no_capset = CapabilitySet::empty();
assert!(!no_capset.has(Capabilities::CHOWN));
```

---

#### has()

```rust
pub fn has(&self, cap: Capabilities) -> bool
```

检查是否拥有某个能力。

**参数**：
- `cap` - 要检查的能力

**返回值**：
- `true` - 拥有该能力
- `false` - 不拥有该能力

**检查逻辑**：检查 effective 能力集

**示例**：
```rust
let capset = CapabilitySet::full();
if capset.has(Capabilities::NET_BIND_SERVICE) {
    // 可以绑定特权端口
    bind_port(80);
}
```

---

#### has_all()

```rust
pub fn has_all(&self, caps: Capabilities) -> bool
```

检查是否拥有所有指定的能力。

**参数**：
- `caps` - 要检查的能力集合

**返回值**：
- `true` - 拥有所有指定能力
- `false` - 缺少至少一个能力

**示例**：
```rust
let required = Capabilities::NET_BIND_SERVICE | Capabilities::NET_ADMIN;
if capset.has_all(required) {
    // 拥有所有网络管理权限
}
```

---

#### add()

```rust
pub fn add(&mut self, cap: Capabilities)
```

添加能力到 effective 和 permitted 集合。

**参数**：
- `cap` - 要添加的能力

**注意**：在单 root 用户系统中，此操作可能无实际效果

**示例**：
```rust
let mut capset = CapabilitySet::empty();
capset.add(Capabilities::CHOWN);
assert!(capset.has(Capabilities::CHOWN));
```

---

#### remove()

```rust
pub fn remove(&mut self, cap: Capabilities)
```

从 effective 集合中移除能力。

**参数**：
- `cap` - 要移除的能力

**注意**：在单 root 用户系统中，此操作可能无实际效果

**示例**：
```rust
let mut capset = CapabilitySet::full();
capset.remove(Capabilities::SYS_ADMIN);
assert!(!capset.has(Capabilities::SYS_ADMIN));
```

---

### 辅助函数

#### capability_from_u32()

```rust
pub fn capability_from_u32(cap_index: u32) -> Option<Capabilities>
```

从能力索引转换为能力位。

**参数**：
- `cap_index` - 能力索引（0-40）

**返回值**：
- `Some(Capabilities)` - 成功转换
- `None` - 索引超出范围

**示例**：
```rust
let cap = capability_from_u32(CAP_CHOWN).unwrap();
assert_eq!(cap, Capabilities::CHOWN);
```

## 使用场景

### 1. 权限检查

```rust
// 检查进程是否有权限绑定特权端口
pub fn bind_privileged_port(port: u16) -> Result<(), Error> {
    let task = current_task();
    let capset = task.lock().capabilities;

    if port < 1024 && !capset.has(Capabilities::NET_BIND_SERVICE) {
        return Err(Error::PermissionDenied);
    }

    // 执行绑定操作
    Ok(())
}
```

### 2. 文件操作权限

```rust
// 检查是否可以改变文件所有者
pub fn chown(path: &str, uid: u32, gid: u32) -> Result<(), Error> {
    let task = current_task();
    let capset = task.lock().capabilities;

    // 需要 CHOWN 能力
    if !capset.has(Capabilities::CHOWN) {
        return Err(Error::PermissionDenied);
    }

    // 执行 chown 操作
    Ok(())
}
```

### 3. 系统管理操作

```rust
// 检查是否可以重启系统
pub fn sys_reboot() -> Result<(), Error> {
    let task = current_task();
    let capset = task.lock().capabilities;

    if !capset.has(Capabilities::SYS_BOOT) {
        return Err(Error::PermissionDenied);
    }

    // 执行重启
    Ok(())
}
```

### 4. 降低权限

```rust
// 进程主动降低权限
pub fn drop_privileges() {
    let task = current_task();
    let mut task_lock = task.lock();

    // 移除不需要的能力
    task_lock.capabilities.remove(Capabilities::SYS_ADMIN);
    task_lock.capabilities.remove(Capabilities::SYS_MODULE);

    // 现在进程只保留必要的能力
}
```

## 实现细节

### 能力存储

- **位标志**：使用 `u64` 存储 41 个能力位
- **bitflags 宏**：提供类型安全的位操作
- **常量定义**：提供与 Linux 兼容的常量（CAP_CHOWN 等）

### 能力继承

当前实现的继承规则（简化版）：
```
fork():
  子进程继承父进程的所有能力集

execve():
  新程序的能力 = (inheritable & file_inheritable) | ambient
```

### 权限检查

内核在执行特权操作前检查 effective 能力集：
```rust
if !current_task().lock().capabilities.has(required_cap) {
    return Err(Error::PermissionDenied);
}
```

### 单 root 用户系统

当前 SanktaOS 是单 root 用户系统：
- 所有进程默认拥有所有能力
- 能力检查总是返回 true
- 为未来的多用户支持预留了接口

## 与 Linux 的对比

| 特性 | SanktaOS | Linux |
|------|----------|-------|
| 能力数量 | 41 个 | 41 个（相同）|
| 能力集数量 | 5 个 | 5 个（相同）|
| 文件能力 | 未实现 | 支持 |
| 能力继承 | 简化版 | 完整实现 |
| 用户命名空间 | 未实现 | 支持 |
| 安全模块集成 | 未实现 | 支持（SELinux、AppArmor）|
| 系统调用 | 未实现 | capget/capset/prctl |

## 已知限制

1. **单 root 用户系统**：所有进程默认拥有所有能力，能力检查形同虚设
2. **缺少文件能力**：不支持为可执行文件设置能力
3. **简化的继承规则**：execve() 时的能力继承规则不完整
4. **缺少系统调用**：没有实现 capget/capset/prctl 等系统调用
5. **无用户命名空间**：不支持用户命名空间中的能力映射
6. **无安全模块集成**：不支持与 SELinux、AppArmor 等安全模块集成

## 未来改进方向

1. **实现多用户支持**：
   - 区分 root 和普通用户
   - 根据 UID 设置初始能力
   - 实现真正的权限检查

2. **实现文件能力**：
   ```rust
   pub struct FileCapabilities {
       permitted: Capabilities,
       inheritable: Capabilities,
       effective: bool,
   }
   ```

3. **完善能力继承**：
   - 实现完整的 execve() 继承规则
   - 支持 setuid/setgid 程序的能力处理

4. **实现系统调用**：
   - `sys_capget()` - 获取进程能力
   - `sys_capset()` - 设置进程能力
   - `sys_prctl()` - 修改能力相关设置

5. **添加能力审计**：
   - 记录能力使用情况
   - 检测异常的能力使用

6. **支持用户命名空间**：
   - 在命名空间中映射能力
   - 实现容器化支持

## 常见能力组合

### Web 服务器

```rust
let web_server_caps = Capabilities::NET_BIND_SERVICE  // 绑定 80/443 端口
                    | Capabilities::DAC_READ_SEARCH;  // 读取文件
```

### 网络管理工具

```rust
let network_tool_caps = Capabilities::NET_ADMIN       // 配置网络
                      | Capabilities::NET_RAW;        // 使用原始套接字
```

### 系统监控工具

```rust
let monitor_caps = Capabilities::SYS_PTRACE           // 跟踪进程
                 | Capabilities::SYSLOG               // 读取内核日志
                 | Capabilities::PERFMON;             // 性能监控
```

## 参考

- 源代码：`os/src/kernel/task/cap.rs`
- Linux 文档：
  - `man 7 capabilities`
  - `man 2 capget`
  - `man 2 capset`
- 相关标准：
  - POSIX.1e (withdrawn)
  - Linux Security Modules (LSM)
