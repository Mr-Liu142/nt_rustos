// SBI API封装
// 提供统一的SBI调用接口

use sbi_rt::legacy;

/// SBI调用返回值类型
pub type SbiResult = Result<usize, SbiError>;

/// SBI错误类型
#[derive(Debug, Copy, Clone)]
pub enum SbiError {
    Failed,
    NotSupported,
    InvalidParam,
    Denied,
    InvalidAddress,
    AlreadyAvailable,
    AlreadyStarted,
    AlreadyStopped,
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

/// 系统相关的SBI调用封装
pub mod system {
    use super::*;

    /// 关闭系统
    pub fn shutdown() -> ! {
        legacy::shutdown();
    }

    /// 重启系统
    pub fn reset() -> ! {
        // 注意: legacy SBI可能不支持reset，这里使用shutdown作为fallback
        legacy::shutdown();
    }
}

/// 时间相关的SBI调用封装
pub mod timer {
    use super::*;

    /// 设置定时器
    /// 
    /// # 参数
    /// * `time` - 定时器触发的时间
    pub fn set_timer(time: u64) {
        legacy::set_timer(time);
    }
}

/// 通用SBI调用接口
/// 
/// # 参数
/// * `eid` - Extension ID
/// * `fid` - Function ID  
/// * `args` - 参数数组
/// 
/// # 返回值
/// SBI调用的返回值
pub fn sbi_call(eid: usize, fid: usize, args: [usize; 6]) -> SbiResult {
    // 这里可以根据需要实现更底层的SBI调用
    // 目前主要使用legacy接口
    Ok(0)
}