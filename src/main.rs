// nt_rustos/src/main.rs

#![no_std]
#![no_main]

use core::arch::asm;
use nt_rustos::{STACK_SIZE, clear_bss, init, main_loop, MemoryInfo, get_memory_info, println, info_print, error_print, debug_print};

// 用于存放栈的内存区域
#[link_section = ".bss.stack"]
static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

/// 程序入口点 - 汇编代码会跳转到这里
#[no_mangle]
#[link_section = ".text.entry"]
fn _start() -> ! {
    // 关键：首先设置栈指针，这样我们才能执行Rust代码
    // 栈向下增长，所以sp指向高地址
    let stack_top = unsafe { STACK.as_ptr().add(STACK_SIZE) as usize };
    unsafe {
        asm!(
            "mv sp, {0}",
            in(reg) stack_top,
            options(nostack) // 确保编译器不在此处插入栈操作
        );
    }

    // 获取栈底，用于BSS清理
    let stack_bottom = unsafe { STACK.as_ptr() as usize };

    // 关键：安全地清空BSS段，同时绕过栈区域。
    unsafe {
        clear_bss(stack_bottom, stack_top);
    }

    // 调用Rust主函数
    rust_main();
}

/// Rust主函数 - 系统的真正入口点
#[no_mangle]
fn rust_main() -> ! {
    // 早期初始化阶段 - 在分配器和trap系统初始化前的基础设置
    // 主要用于设置控制台输出等，以便后续打印信息。
    // 此阶段不应有任何需要内存分配或复杂错误处理的操作。
    early_printk_banner();

    // 系统核心初始化 - 包括分配器和trap子系统
    init(); // 此函数现在会初始化分配器和trap系统

    // 验证系统状态
    verify_system_state_after_init();

    // 进入主循环
    main_loop();
}

/// 早期打印Banner信息
fn early_printk_banner() {
    // 此时控制台应该可用（通过SBI），但不依赖格式化宏
    console::print_str("\nNT RustOS Booting...\n");
    console::print_str("Version: 0.2.0 (Trap System Refactored)\n");
    console::print_str("Architecture: RISC-V 64-bit\n");
    let stack_top = unsafe { STACK.as_ptr().add(STACK_SIZE) as usize };
    let stack_bottom = unsafe { STACK.as_ptr() as usize };
    console::print_str("Stack Range (setup): 0x");
    console::print_hex(stack_bottom);
    console::print_str(" - 0x");
    console::print_hex(stack_top);
    console::print_str(" (");
    console::print_num(STACK_SIZE / 1024);
    console::print_str(" KB)\n");

    // 获取内核边界信息
    extern "C" {
        fn sbss();
        fn ebss();
        fn end(); // end通常指 .bss段之后，堆开始之前
    }
    console::print_str("Kernel .bss segment: 0x");
    console::print_hex(sbss as usize);
    console::print_str(" - 0x");
    console::print_hex(ebss as usize);
    console::print_str("\n");
    console::print_str("Kernel end symbol: 0x");
    console::print_hex(end as usize);
    console::print_str("\n");
}


/// 验证系统初始化后的状态
fn verify_system_state_after_init() {
    info_print!("Verifying system state post-initialization...");

    // 检查分配器状态
    if !init::alloc::is_initialized() {
        error_print!("FATAL: Allocator failed to initialize properly!");
        panic!("System verification failed: Allocator not initialized.");
    }
    if !init::alloc::is_enabled() {
        warn_print!("Allocator is initialized but not enabled.");
    } else {
        info_print!("Allocator state: Initialized and Enabled.");
    }

    // 检查Trap系统状态
    if !trap::infrastructure::di::is_initialized() {
        error_print!("FATAL: Trap subsystem failed to initialize properly!");
        panic!("System verification failed: Trap system not initialized.");
    } else {
        info_print!("Trap subsystem state: Initialized.");
    }


    // 获取并打印内存信息
    let memory_info = get_memory_info();
    memory_info.print();

    if !memory_info.is_healthy() {
        warn_print!("Memory system health check reported issues post-init.");
    } else {
        info_print!("Memory system health check: PASSED.");
    }

    // 执行分配器完整性检查
    match init::alloc::integrity_check() {
        Ok(_) => { info_print!("Allocator integrity check: PASSED."); }
        Err(e) => {
            error_print!("Allocator integrity check failed: {:?}", e);
            panic!("System verification failed: Allocator integrity compromised.");
        }
    }

    info_print!("System state verification completed successfully.");
}