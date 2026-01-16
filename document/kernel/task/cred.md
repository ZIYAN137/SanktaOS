# 凭证管理（Credentials）

## 概述

凭证（Credentials）是进程的身份标识，包含用户 ID（UID）、组 ID（GID）和能力集合。内核使用凭证来进行权限检查，决定进程是否有权执行特定操作。

SanktaOS 实现了与 Linux 兼容的凭证系统，支持多种 UID/GID 类型和能力管理。

**代码位置**：`os/src/kernel/task/cred.rs`

## 核心概念

### UID/GID 的多种类型

Linux 为每个进程维护多组 UID/GID，用于不同的目的：

1. **Real UID/GID（真实 UID/GID）**：
   - 标识进程的实际所有者
   - 用于信号发送权限检查
   - 通常不会改变

2. **Effective UID/GID（有效 UID/GID）**：
   - 用于大多数权限检查
   - 决定进程的实际权限
   - 可以通过 setuid/setgid 程序临时改变

3. **Saved UID/GID（保存的 UID/GID）**：
   - 保存 setuid/setgid 程序的原始权限
   - 允许进程在特权和非特权之间切换
   - 用于实现权限降低和恢复

4. **Filesystem UID/GID（文件系统 UID/GID）**：
   - Linux 扩展，专门用于文件系统操作
   - 通常与 effective UID/GID 相同
   - 允许 NFS 服务器等程序独立控制文件系统权限

### Setuid/Setgid 程序

Setuid/Setgid 是 Unix 的重要特性：
- **Setuid 程序**：执行时，effective UID 变为文件所有者的 UID
- **Setgid 程序**：执行时，effective GID 变为文件所属组的 GID
- **典型应用**：`passwd` 命令需要修改 `/etc/shadow`，但普通用户无权访问

**工作流程**：
```
普通用户执行 /usr/bin/passwd (setuid root):
  1. Real UID = 1000 (普通用户)
  2. Effective UID = 0 (root，来自文件所有者)
  3. Saved UID = 0 (保存 root 权限)
  4. 现在可以修改 /etc/shadow
  5. 完成后可以降低权限：setuid(1000)
```

## 核心数据结构

### Credential

进程凭证结构，包含所有身份信息。

```rust
pub struct Credential {
    pub uid: u32,
    pub gid: u32,
    pub euid: u32,
    pub egid: u32,
    pub suid: u32,
    pub sgid: u32,
    pub fsuid: u32,
    pub fsgid: u32,
    pub capabilities: CapabilitySet,
}
```

**字段说明**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `uid` | u32 | 真实用户 ID |
| `gid` | u32 | 真实组 ID |
| `euid` | u32 | 有效用户 ID（权限检查）|
| `egid` | u32 | 有效组 ID（权限检查）|
| `suid` | u32 | 保存的用户 ID（setuid 程序）|
| `sgid` | u32 | 保存的组 ID（setgid 程序）|
| `fsuid` | u32 | 文件系统用户 ID |
| `fsgid` | u32 | 文件系统组 ID |
| `capabilities` | CapabilitySet | 能力集合 |

## API 参考

### Credential::root()

```rust
pub const fn root() -> Self
```

创建 root 用户凭证。

**返回值**：所有 UID/GID 都为 0，拥有所有能力的凭证

**示例**：
```rust
let root_cred = Credential::root();
assert_eq!(root_cred.uid, 0);
assert_eq!(root_cred.euid, 0);
assert!(root_cred.is_root());
```

---

### Credential::is_root()

```rust
pub fn is_root(&self) -> bool
```

检查是否为 root 用户。

**返回值**：
- `true` - effective UID 为 0（root）
- `false` - 非 root 用户

**检查逻辑**：只检查 effective UID，不检查 real UID

**示例**：
```rust
let cred = current_task().lock().credential;
if cred.is_root() {
    // 执行需要 root 权限的操作
}
```

## 使用场景

### 1. 权限检查

```rust
// 检查是否有权限访问文件
pub fn check_file_access(file_uid: u32, file_mode: u32) -> Result<(), Error> {
    let cred = current_task().lock().credential;

    // root 用户可以访问任何文件
    if cred.is_root() {
        return Ok(());
    }

    // 检查文件所有者
    if cred.fsuid == file_uid {
        // 检查所有者权限位
        if file_mode & 0o400 != 0 {
            return Ok(());
        }
    }

    Err(Error::PermissionDenied)
}
```

### 2. 信号发送权限

```rust
// 检查是否可以向目标进程发送信号
pub fn can_send_signal(target_pid: i32) -> bool {
    let sender_cred = current_task().lock().credential;
    let target_task = find_task(target_pid)?;
    let target_cred = target_task.lock().credential;

    // root 可以向任何进程发送信号
    if sender_cred.is_root() {
        return true;
    }

    // 检查 real UID 或 effective UID 是否匹配
    sender_cred.uid == target_cred.uid || sender_cred.euid == target_cred.uid
}
```

### 3. Setuid 程序执行

```rust
// 执行 setuid 程序
pub fn exec_setuid_program(file_uid: u32) {
    let task = current_task();
    let mut task_lock = task.lock();
    let mut cred = task_lock.credential;

    // 保存当前 effective UID
    cred.suid = cred.euid;

    // 设置新的 effective UID
    cred.euid = file_uid;
    cred.fsuid = file_uid;

    // 如果变成 root，获得所有能力
    if file_uid == 0 {
        cred.capabilities = CapabilitySet::full();
    }

    task_lock.credential = cred;
}
```

### 4. 权限降低和恢复

```rust
// 临时降低权限
pub fn drop_privileges() {
    let task = current_task();
    let mut task_lock = task.lock();
    let mut cred = task_lock.credential;

    // 降低到 real UID
    cred.euid = cred.uid;
    cred.fsuid = cred.uid;

    task_lock.credential = cred;
}

// 恢复权限（setuid 程序）
pub fn restore_privileges() {
    let task = current_task();
    let mut task_lock = task.lock();
    let mut cred = task_lock.credential;

    // 恢复到 saved UID
    cred.euid = cred.suid;
    cred.fsuid = cred.suid;

    task_lock.credential = cred;
}
```

## 实现细节

### 凭证存储

- **任务结构**：每个任务（进程/线程）都有自己的 `Credential` 副本
- **Copy 语义**：`Credential` 实现了 `Copy` trait，修改时直接复制
- **不可变性**：凭证修改需要获取任务锁

### 初始化

- **Init 进程**：第一个进程使用 `Credential::root()`
- **Fork**：子进程继承父进程的凭证
- **Exec**：根据文件的 setuid/setgid 位调整凭证

### 权限检查顺序

内核进行权限检查时的典型顺序：
1. 检查 effective UID 是否为 0（root）
2. 检查能力集合
3. 检查文件所有者和权限位
4. 检查组权限
5. 检查其他用户权限

### 与能力系统的关系

- **Root 用户**：euid == 0 时，自动拥有所有能力
- **非 Root 用户**：可以拥有部分能力，实现细粒度权限控制
- **Setuid Root**：执行 setuid root 程序时，获得所有能力

## 与 Linux 的对比

| 特性 | SanktaOS | Linux |
|------|----------|-------|
| UID/GID 类型 | 8 种（完整）| 8 种（相同）|
| Root 用户 | UID = 0 | UID = 0（相同）|
| Setuid/Setgid | 基础支持 | 完整支持 |
| 补充组 | 未实现 | 支持（最多 65536 个）|
| 用户命名空间 | 未实现 | 支持 |
| 凭证共享 | 未实现 | 支持（线程间）|
| 系统调用 | 部分实现 | 完整（setuid/setgid/setreuid 等）|

## 已知限制

1. **单 Root 用户系统**：当前所有进程都以 root 运行，凭证检查形同虚设
2. **缺少补充组**：不支持补充组列表（supplementary groups）
3. **简化的 Setuid 处理**：setuid/setgid 程序的处理逻辑不完整
4. **缺少系统调用**：未实现完整的 setuid/setgid/setreuid/setresuid 等系统调用
5. **无用户命名空间**：不支持用户命名空间中的 UID/GID 映射
6. **无凭证共享**：线程间不共享凭证（Linux 中可以共享）

## 未来改进方向

1. **实现多用户支持**：
   - 支持创建普通用户
   - 实现用户登录和认证
   - 根据用户设置初始凭证

2. **实现补充组**：
   ```rust
   pub struct Credential {
       // ... 现有字段
       pub groups: Vec<u32>,  // 补充组列表
   }
   ```

3. **完善 Setuid/Setgid 处理**：
   - 实现完整的 execve() 凭证转换逻辑
   - 处理文件能力和凭证的交互
   - 实现安全的权限降低机制

4. **实现完整的系统调用**：
   - `sys_setuid()` / `sys_setgid()` - 设置 UID/GID
   - `sys_setreuid()` / `sys_setregid()` - 设置 real 和 effective UID/GID
   - `sys_setresuid()` / `sys_setresgid()` - 设置所有三种 UID/GID
   - `sys_setfsuid()` / `sys_setfsgid()` - 设置文件系统 UID/GID
   - `sys_getuid()` / `sys_getgid()` - 获取 UID/GID
   - `sys_geteuid()` / `sys_getegid()` - 获取 effective UID/GID

5. **支持用户命名空间**：
   - 在命名空间中映射 UID/GID
   - 实现容器化支持

6. **实现凭证共享**：
   - 允许线程间共享凭证
   - 使用引用计数管理凭证

7. **添加审计支持**：
   - 记录凭证变更
   - 检测异常的权限提升

## 常见凭证场景

### 普通用户

```rust
Credential {
    uid: 1000,
    gid: 1000,
    euid: 1000,
    egid: 1000,
    suid: 1000,
    sgid: 1000,
    fsuid: 1000,
    fsgid: 1000,
    capabilities: CapabilitySet::empty(),
}
```

### Setuid Root 程序

```rust
// 执行前（普通用户）
Credential {
    uid: 1000,      // 真实用户
    euid: 1000,     // 有效用户
    suid: 1000,     // 保存的用户
}

// 执行后（setuid root）
Credential {
    uid: 1000,      // 真实用户不变
    euid: 0,        // 有效用户变为 root
    suid: 0,        // 保存 root 权限
}
```

### 守护进程

```rust
// 以 root 启动，然后降低权限
Credential {
    uid: 0,         // 启动时为 root
    euid: 65534,    // 降低到 nobody
    suid: 0,        // 保留 root 权限以便恢复
}
```

## 安全考虑

### 1. 权限提升攻击

- **风险**：Setuid 程序可能被利用提升权限
- **防护**：
  - 最小化 setuid 程序数量
  - 仔细审查 setuid 程序代码
  - 使用能力系统代替 setuid

### 2. 权限降低

- **最佳实践**：
  ```rust
  // 永久降低权限
  cred.uid = 1000;
  cred.euid = 1000;
  cred.suid = 1000;  // 清除保存的权限
  ```

### 3. 文件系统操作

- **注意**：使用 `fsuid` 而不是 `euid` 进行文件系统权限检查
- **原因**：允许 NFS 服务器等程序独立控制文件系统权限

## 参考

- 源代码：`os/src/kernel/task/cred.rs`
- 相关模块：
  - 能力管理：`kernel::task::cap`
  - 任务管理：`kernel::task`
- Linux 文档：
  - `man 7 credentials`
  - `man 2 setuid`
  - `man 2 setreuid`
  - `man 2 setresuid`
- 相关标准：
  - POSIX.1-2001
  - Linux-specific extensions
