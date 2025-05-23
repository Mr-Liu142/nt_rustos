#![no_std]
#![no_main]
#![feature(panic_info_message)]

use core::panic::PanicInfo;
use core::arch::asm;

mod console;
mod util;

// 启动栈大小 (16KB)
const STACK_SIZE: usize = 4096 * 4;

// 用于存放栈的内存区域
#[link_section = ".bss.stack"]
static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

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

/// 程序入口点 - 汇编代码会跳转到这里
#[no_mangle]
#[link_section = ".text.entry"]
fn _start() -> ! {
    unsafe {
        // 设置栈指针到栈顶
        let stack_top = STACK.as_ptr().add(STACK_SIZE);
        asm!(
            "mv sp, {0}",
            in(reg) stack_top,
        );
        
        // 清除BSS段 - 将未初始化的静态变量清零
        clear_bss();
        
        // 跳转到Rust主函数
        rust_main();
    }
}

/// 清除BSS段
unsafe fn clear_bss() {
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

/// Rust主函数 - 系统的真正入口点
#[no_mangle]
fn rust_main() -> ! {
    // 系统初始化
    init();
    
    // 主循环
    main_loop();
}

/// 系统初始化
fn init() {
    info_print!("NT RustOS starting...");
    info_print!("Stack size: {} bytes", STACK_SIZE);
    info_print!("BSS cleared successfully");
    info_print!("System initialization completed");
}

/// 主循环 - 系统的核心循环
fn main_loop() -> ! {
    info_print!("Entering main loop");
    
    // 测试不同的输出功能
    test_console_output();
    
    // 主循环 - 目前只是等待中断
    loop {
        unsafe {
            asm!("wfi"); // 等待中断，降低功耗
        }
    }
}

/// 测试控制台输出功能
fn test_console_output() {
    println!("=== Console Output Test ===");
    
    // 测试基本输出
    println!("Hello, RISC-V World!");
    
    // 测试数字输出
    println!("Decimal: {}", 42);
    println!("Hexadecimal: 0x{:x}", 255);
    println!("Octal: 0o{:o}", 64);
    
    // 测试直接数字输出函数
    console::print_str("Direct number output: ");
    console::print_num(12345);
    console::print_str("\n");
    
    console::print_str("Hex number: ");
    console::print_hex(0xdeadbeef);
    console::print_str("\n");
    
    console::print_str("Oct number: ");
    console::print_oct(0o777);
    console::print_str("\n");
    
    // 测试彩色输出
    info_print!("This is an info message");
    warn_print!("This is a warning message");
    error_print!("This is an error message");
    
    // 测试调试输出
    debug_print!("This is a debug message");
    
    println!("=== Test completed ===");
}