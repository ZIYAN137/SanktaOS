//! 网络设备接口定义

/// 网络设备错误
#[derive(Debug)]
pub enum NetDeviceError {
    /// IO 错误
    IoError,
    /// 设备未就绪
    DeviceNotReady,
    /// 不支持的操作
    NotSupported,
    /// 队列已满
    QueueFull,
    /// 队列为空
    QueueEmpty,
    /// 分配失败
    AllocationFailed,
}

/// 网络设备接口
pub trait NetDevice: Send + Sync {
    /// 发送数据包
    fn send(&self, packet: &[u8]) -> Result<(), NetDeviceError>;

    /// 接收数据包
    fn receive(&self, buf: &mut [u8]) -> Result<usize, NetDeviceError>;

    /// 获取设备标识符
    fn device_id(&self) -> usize;

    /// 获取最大传输单元(MTU)
    fn mtu(&self) -> usize;

    /// 获取设备名称
    fn name(&self) -> &str;

    /// 获取MAC地址
    fn mac_address(&self) -> [u8; 6];
}
