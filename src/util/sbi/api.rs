// SBI API完整封装
// 基于RISC-V SBI v2.0规范提供全面的SBI调用接口

use sbi_rt::legacy;

/// SBI调用返回值类型
pub type SbiResult = Result<usize, SbiError>;

/// SBI错误类型 - 符合SBI规范的错误代码
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SbiError {
    Success = 0,
    Failed = -1,
    NotSupported = -2,
    InvalidParam = -3,
    Denied = -4,
    InvalidAddress = -5,
    AlreadyAvailable = -6,
    AlreadyStarted = -7,
    AlreadyStopped = -8,
}

/// SBI扩展ID常量 - 符合SBI规范定义
pub mod extension_ids {
    pub const BASE: usize = 0x10;
    pub const TIMER: usize = 0x54494D45;  // "TIME"
    pub const IPI: usize = 0x735049;      // "sPI"  
    pub const RFENCE: usize = 0x52464E43; // "RFNC"
    pub const HSM: usize = 0x48534D;      // "HSM"
    pub const SRST: usize = 0x53525354;   // "SRST"
    pub const PMU: usize = 0x504D55;      // "PMU"
    pub const DBCN: usize = 0x4442434E;   // "DBCN"
    pub const SUSP: usize = 0x53555350;   // "SUSP"
    pub const CPPC: usize = 0x43505043;   // "CPPC"
    pub const NACL: usize = 0x4E41434C;   // "NACL"
    pub const STA: usize = 0x535441;      // "STA"
}

/// 基础SBI调用
pub mod base {
    use super::*;

    /// 获取SBI规范版本
    pub fn get_spec_version() -> SbiResult {
        let ret = sbi_call(extension_ids::BASE, 0, [0; 6]);
        Ok(ret.unwrap_or(0))
    }

    /// 获取SBI实现ID
    pub fn get_impl_id() -> SbiResult {
        let ret = sbi_call(extension_ids::BASE, 1, [0; 6]);
        Ok(ret.unwrap_or(0))
    }

    /// 获取SBI实现版本
    pub fn get_impl_version() -> SbiResult {
        let ret = sbi_call(extension_ids::BASE, 2, [0; 6]);
        Ok(ret.unwrap_or(0))
    }

    /// 探测SBI扩展是否可用
    pub fn probe_extension(extension_id: usize) -> SbiResult {
        let ret = sbi_call(extension_ids::BASE, 3, [extension_id, 0, 0, 0, 0, 0]);
        Ok(ret.unwrap_or(0))
    }

    /// 获取CPU厂商ID
    pub fn get_mvendorid() -> SbiResult {
        let ret = sbi_call(extension_ids::BASE, 4, [0; 6]);
        Ok(ret.unwrap_or(0))
    }

    /// 获取CPU架构ID  
    pub fn get_marchid() -> SbiResult {
        let ret = sbi_call(extension_ids::BASE, 5, [0; 6]);
        Ok(ret.unwrap_or(0))
    }

    /// 获取CPU实现ID
    pub fn get_mimpid() -> SbiResult {
        let ret = sbi_call(extension_ids::BASE, 6, [0; 6]);
        Ok(ret.unwrap_or(0))
    }
}

/// 控制台相关的SBI调用封装
pub mod console {
    use super::*;

    /// 输出单个字符到控制台
    /// 
    /// # 参数
    /// * `ch` - 要输出的字符
    /// 
    /// # 返回值
    /// 总是返回Ok(0)，因为legacy console_putchar不会失败
    pub fn putchar(ch: char) -> SbiResult {
        legacy::console_putchar(ch as usize);
        Ok(0)
    }

    /// 从控制台读取字符(如果支持)
    pub fn getchar() -> SbiResult {
        // 注意：getchar在很多SBI实现中不支持
        // 这里只是提供接口，实际可能返回NotSupported
        Err(SbiError::NotSupported)
    }

    /// 输出字符串到控制台
    /// 
    /// # 参数
    /// * `s` - 要输出的字符串
    /// 
    /// # 返回值
    /// 成功输出的字符数
    pub fn puts(s: &str) -> SbiResult {
        let mut count = 0;
        for ch in s.chars() {
            putchar(ch)?;
            count += 1;
        }
        Ok(count)
    }

    /// 输出数字到控制台
    /// 
    /// # 参数
    /// * `num` - 要输出的数字
    /// * `base` - 进制 (8, 10, 16)
    /// 
    /// # 返回值
    /// 成功输出的字符数
    pub fn putnum(num: usize, base: usize) -> SbiResult {
        if base != 8 && base != 10 && base != 16 {
            return Err(SbiError::InvalidParam);
        }

        if num == 0 {
            putchar('0')?;
            return Ok(1);
        }

        let mut n = num;
        let mut buf = [0u8; 64]; // 足够存储任何64位数字
        let mut i = 0;
        
        // 使用切片来避免类型不匹配问题
        let digits: &[u8] = match base {
            16 => b"0123456789abcdef",
            10 => b"0123456789",
            8 => b"01234567",
            _ => return Err(SbiError::InvalidParam),
        };

        // 转换数字为字符串
        while n > 0 {
            buf[i] = digits[n % base];
            n /= base;
            i += 1;
        }

        // 如果是16进制，添加0x前缀
        let mut count = 0;
        if base == 16 {
            putchar('0')?;
            putchar('x')?;
            count += 2;
        } else if base == 8 {
            // 8进制添加0前缀
            putchar('0')?;
            count += 1;
        }

        // 反向输出数字
        while i > 0 {
            i -= 1;
            putchar(buf[i] as char)?;
            count += 1;
        }

        Ok(count)
    }
}

/// 时间相关的SBI调用封装
pub mod timer {
    use super::*;

    /// 设置定时器
    /// 
    /// # 参数
    /// * `time` - 定时器触发的时间
    pub fn set_timer(time: u64) -> SbiResult {
        legacy::set_timer(time);
        Ok(0)
    }
}

/// 处理器间中断(IPI)扩展
pub mod ipi {
    use super::*;

    /// 发送IPI到指定的hart
    /// 
    /// # 参数
    /// * `hart_mask` - 目标hart掩码
    pub fn send_ipi(hart_mask: usize) -> SbiResult {
        let ret = sbi_call(extension_ids::IPI, 0, [hart_mask, 0, 0, 0, 0, 0]);
        match ret {
            Ok(0) => Ok(0),
            _ => Err(SbiError::Failed),
        }
    }
}

/// 远程fence扩展
pub mod rfence {
    use super::*;

    /// 远程fence.i指令
    pub fn remote_fence_i(hart_mask: usize) -> SbiResult {
        let ret = sbi_call(extension_ids::RFENCE, 0, [hart_mask, 0, 0, 0, 0, 0]);
        match ret {
            Ok(0) => Ok(0),
            _ => Err(SbiError::Failed),
        }
    }

    /// 远程sfence.vma指令
    pub fn remote_sfence_vma(hart_mask: usize, start: usize, size: usize) -> SbiResult {
        let ret = sbi_call(extension_ids::RFENCE, 1, [hart_mask, start, size, 0, 0, 0]);
        match ret {
            Ok(0) => Ok(0),
            _ => Err(SbiError::Failed),
        }
    }

    /// 远程sfence.vma.asid指令
    pub fn remote_sfence_vma_asid(hart_mask: usize, start: usize, size: usize, asid: usize) -> SbiResult {
        let ret = sbi_call(extension_ids::RFENCE, 2, [hart_mask, start, size, asid, 0, 0]);
        match ret {
            Ok(0) => Ok(0),
            _ => Err(SbiError::Failed),
        }
    }
}

/// Hart状态管理(HSM)扩展
pub mod hsm {
    use super::*;

    /// Hart状态常量
    pub const HART_STATE_STARTED: usize = 0;
    pub const HART_STATE_STOPPED: usize = 1;
    pub const HART_STATE_START_REQUEST_PENDING: usize = 2;
    pub const HART_STATE_STOP_REQUEST_PENDING: usize = 3;

    /// 启动一个hart
    /// 
    /// # 参数
    /// * `hartid` - 目标hart ID
    /// * `start_addr` - 启动地址
    /// * `opaque` - 传递给hart的参数
    pub fn hart_start(hartid: usize, start_addr: usize, opaque: usize) -> SbiResult {
        let ret = sbi_call(extension_ids::HSM, 0, [hartid, start_addr, opaque, 0, 0, 0]);
        match ret {
            Ok(0) => Ok(0),
            _ => Err(SbiError::Failed),
        }
    }

    /// 停止当前hart
    pub fn hart_stop() -> SbiResult {
        let ret = sbi_call(extension_ids::HSM, 1, [0; 6]);
        match ret {
            Ok(0) => Ok(0),
            _ => Err(SbiError::Failed),
        }
    }

    /// 获取hart状态
    /// 
    /// # 参数
    /// * `hartid` - 目标hart ID
    /// 
    /// # 返回值
    /// Hart状态值
    pub fn hart_get_status(hartid: usize) -> SbiResult {
        let ret = sbi_call(extension_ids::HSM, 2, [hartid, 0, 0, 0, 0, 0]);
        ret
    }
}

/// 系统重置扩展
pub mod system_reset {
    use super::*;

    /// 重置类型常量
    pub const RESET_TYPE_SHUTDOWN: usize = 0;
    pub const RESET_TYPE_COLD_REBOOT: usize = 1;
    pub const RESET_TYPE_WARM_REBOOT: usize = 2;

    /// 重置原因常量
    pub const RESET_REASON_NO_REASON: usize = 0;
    pub const RESET_REASON_SYSTEM_FAILURE: usize = 1;

    /// 系统重置
    /// 
    /// # 参数
    /// * `reset_type` - 重置类型
    /// * `reset_reason` - 重置原因
    pub fn system_reset(reset_type: usize, reset_reason: usize) -> ! {
        let _ = sbi_call(extension_ids::SRST, 0, [reset_type, reset_reason, 0, 0, 0, 0]);
        // 如果SBI调用失败，使用legacy shutdown
        legacy::shutdown();
    }
}

/// 系统相关的SBI调用封装
pub mod system {
    use super::*;

    /// 关闭系统
    pub fn shutdown() -> ! {
        system_reset::system_reset(
            system_reset::RESET_TYPE_SHUTDOWN,
            system_reset::RESET_REASON_NO_REASON
        );
    }

    /// 冷重启系统
    pub fn reboot() -> ! {
        system_reset::system_reset(
            system_reset::RESET_TYPE_COLD_REBOOT, 
            system_reset::RESET_REASON_NO_REASON
        );
    }

    /// 热重启系统
    pub fn warm_reboot() -> ! {
        system_reset::system_reset(
            system_reset::RESET_TYPE_WARM_REBOOT,
            system_reset::RESET_REASON_NO_REASON
        );
    }
}

/// 性能监控单元(PMU)扩展
pub mod pmu {
    use super::*;

    /// 获取PMU计数器数量
    pub fn get_num_counters() -> SbiResult {
        let ret = sbi_call(extension_ids::PMU, 0, [0; 6]);
        ret
    }

    /// 获取计数器信息
    pub fn get_counter_info(counter_idx: usize) -> SbiResult {
        let ret = sbi_call(extension_ids::PMU, 1, [counter_idx, 0, 0, 0, 0, 0]);
        ret
    }
}

/// 调试控制台扩展
pub mod debug_console {
    use super::*;

    /// 调试控制台写
    pub fn console_write(num_bytes: usize, base_addr_lo: usize, base_addr_hi: usize) -> SbiResult {
        let ret = sbi_call(extension_ids::DBCN, 0, [num_bytes, base_addr_lo, base_addr_hi, 0, 0, 0]);
        ret
    }

    /// 调试控制台读
    pub fn console_read(num_bytes: usize, base_addr_lo: usize, base_addr_hi: usize) -> SbiResult {
        let ret = sbi_call(extension_ids::DBCN, 1, [num_bytes, base_addr_lo, base_addr_hi, 0, 0, 0]);
        ret
    }

    /// 调试控制台写字节
    pub fn console_write_byte(byte: u8) -> SbiResult {
        let ret = sbi_call(extension_ids::DBCN, 2, [byte as usize, 0, 0, 0, 0, 0]);
        ret
    }
}

/// SBI信息查询接口
pub mod info {
    use super::*;

    /// 检查SBI扩展是否可用
    pub fn is_extension_available(extension_id: usize) -> bool {
        match base::probe_extension(extension_id) {
            Ok(0) => false,  // 不可用
            Ok(_) => true,   // 可用
            Err(_) => false, // 错误，假设不可用
        }
    }

    /// 打印SBI系统信息
    pub fn print_sbi_info() {
        console::puts("=== SBI System Information ===\n").ok();
        
        if let Ok(version) = base::get_spec_version() {
            console::puts("SBI Spec Version: ").ok();
            console::putnum(version, 16).ok();
            console::puts("\n").ok();
        }

        if let Ok(impl_id) = base::get_impl_id() {
            console::puts("SBI Implementation ID: ").ok();
            console::putnum(impl_id, 16).ok();
            console::puts("\n").ok();
        }

        if let Ok(impl_ver) = base::get_impl_version() {
            console::puts("SBI Implementation Version: ").ok();
            console::putnum(impl_ver, 16).ok();
            console::puts("\n").ok();
        }

        // 检查各个扩展的可用性
        let extensions = [
            ("Timer", extension_ids::TIMER),
            ("IPI", extension_ids::IPI),
            ("RFENCE", extension_ids::RFENCE),
            ("HSM", extension_ids::HSM),
            ("System Reset", extension_ids::SRST),
            ("PMU", extension_ids::PMU),
            ("Debug Console", extension_ids::DBCN),
        ];

        console::puts("Available Extensions:\n").ok();
        for (name, id) in extensions.iter() {
            console::puts("  ").ok();
            console::puts(name).ok();
            console::puts(": ").ok();
            if is_extension_available(*id) {
                console::puts("Available\n").ok();
            } else {
                console::puts("Not Available\n").ok();
            }
        }
        console::puts("==============================\n").ok();
    }
}

/// 底层SBI调用接口
/// 
/// # 参数
/// * `eid` - Extension ID
/// * `fid` - Function ID  
/// * `args` - 参数数组
/// 
/// # 返回值
/// SBI调用的返回值
pub fn sbi_call(eid: usize, fid: usize, args: [usize; 6]) -> SbiResult {
    let error: isize;
    let value: usize;
    
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,        // Extension ID
            in("a6") fid,        // Function ID  
            in("a0") args[0],    // 参数0
            in("a1") args[1],    // 参数1
            in("a2") args[2],    // 参数2
            in("a3") args[3],    // 参数3
            in("a4") args[4],    // 参数4
            in("a5") args[5],    // 参数5
            lateout("a0") error, // 错误码
            lateout("a1") value, // 返回值
        );
    }
    
    // 根据SBI规范解析返回值
    match error {
        0 => Ok(value),                          // 成功
        -1 => Err(SbiError::Failed),            // 失败
        -2 => Err(SbiError::NotSupported),      // 不支持
        -3 => Err(SbiError::InvalidParam),      // 无效参数
        -4 => Err(SbiError::Denied),            // 拒绝访问
        -5 => Err(SbiError::InvalidAddress),    // 无效地址
        -6 => Err(SbiError::AlreadyAvailable),  // 已经可用
        -7 => Err(SbiError::AlreadyStarted),    // 已经启动
        -8 => Err(SbiError::AlreadyStopped),    // 已经停止
        _ => Err(SbiError::Failed),             // 未知错误
    }
}