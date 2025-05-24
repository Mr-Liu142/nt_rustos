// 控制台输出模块
// 使用封装的SBI API实现控制台功能

use core::fmt;
use crate::util::sbi;

/// 格式化输出函数
pub fn print(args: fmt::Arguments) {
    use core::fmt::Write;
    Stdout.write_fmt(args).unwrap();
}

/// 直接输出字符串
pub fn print_str(s: &str) {
    let _ = sbi::console::puts(s);
}

/// 输出单个字符
pub fn print_char(ch: char) {
    let _ = sbi::console::putchar(ch);
}

/// 输出十进制数字
pub fn print_num(num: usize) {
    let _ = sbi::console::putnum(num, 10);
}

/// 输出十六进制数字
pub fn print_hex(num: usize) {
    let _ = sbi::console::putnum(num, 16);
}

/// 输出八进制数字
pub fn print_oct(num: usize) {
    let _ = sbi::console::putnum(num, 8);
}

/// 标准输出结构体，实现Write trait以支持格式化输出
struct Stdout;

impl core::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        print_str(s);
        Ok(())
    }
}

/// print宏 - 格式化输出
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::console::print(format_args!($($arg)*))
    };
}

/// println宏 - 格式化输出并换行
#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {
        $crate::print!("{}\n", format_args!($($arg)*))
    };
}

/// 调试输出宏 - 带有文件和行号信息
#[macro_export]
macro_rules! debug_print {
    ($($arg:tt)*) => {{
        $crate::print!("[{}:{}] ", file!(), line!());
        $crate::println!($($arg)*);
    }};
}

/// 错误输出宏 - 红色高亮显示
#[macro_export]
macro_rules! error_print {
    ($($arg:tt)*) => {{
        $crate::print!("\x1b[31m[ERROR] ");
        $crate::print!($($arg)*);
        $crate::print!("\x1b[0m\n");
    }};
}

/// 警告输出宏 - 黄色高亮显示
#[macro_export]
macro_rules! warn_print {
    ($($arg:tt)*) => {{
        $crate::print!("\x1b[33m[WARN] ");
        $crate::print!($($arg)*);
        $crate::print!("\x1b[0m\n");
    }};
}

/// 信息输出宏 - 绿色高亮显示
#[macro_export]
macro_rules! info_print {
    ($($arg:tt)*) => {{
        $crate::print!("\x1b[32m[INFO] ");
        $crate::print!($($arg)*);
        $crate::print!("\x1b[0m\n");
    }};
}