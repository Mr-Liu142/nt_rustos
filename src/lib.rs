#![no_std]
#![feature(panic_info_message)]

// 导出核心模块
pub mod console;
pub mod util;
pub mod test;  // 直接包含测试模块，无条件编译

use core::panic::PanicInfo;
use core::arch::asm;

/// 启动栈大小 (16KB)
pub const STACK_SIZE: usize = 4096 * 4;

/// Panic处理器 - 当发生panic时调用
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // 输出panic信息
    error_print!("Kernel panic!");
    
    if let Some(location) = info.location() {
        error_print!("Location: {}:{}", location.file(), location.line());
    }
    
    if let Some(message) = info.message() {
        error_print!("Message: {}", message);
    }
    
    // 无限循环，停止系统
    loop {
        unsafe {
            asm!("wfi"); // 等待中断，降低功耗
        }
    }
}

/// 清除BSS段
pub unsafe fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    
    let sbss_addr = sbss as usize;
    let ebss_addr = ebss as usize;
    
    // 逐字节清零BSS段
    for addr in sbss_addr..ebss_addr {
        core::ptr::write_volatile(addr as *mut u8, 0);
    }
}

/// 系统初始化
pub fn init() {
    info_print!("NT RustOS starting...");
    info_print!("Stack size: {} bytes", STACK_SIZE);
    info_print!("BSS cleared successfully");
    
    // 显示SBI系统信息
    util::sbi::info::print_sbi_info();
    
    info_print!("System initialization completed");
}

/// 主循环 - 系统的核心循环
pub fn main_loop() -> ! {
    info_print!("Entering main loop");
    
    // 运行所有测试
    info_print!("Running tests...");
    test::run_all_tests();
    info_print!("All tests completed");
    
    // 主循环 - 目前只是等待中断
    loop {
        unsafe {
            asm!("wfi"); // 等待中断，降低功耗
        }
    }
}

/// 系统关闭
pub fn shutdown() -> ! {
    info_print!("System shutting down...");
    util::sbi::system::shutdown();
}

/// 系统重启
pub fn reboot() -> ! {
    info_print!("System rebooting...");
    util::sbi::system::reboot();
}