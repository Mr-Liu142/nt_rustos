#![no_std]
#![no_main]

use core::arch::asm;
use nt_rustos::{STACK_SIZE, clear_bss, init, main_loop, MemoryInfo, get_memory_info};

// 用于存放栈的内存区域
#[link_section = ".bss.stack"]
static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

/// 程序入口点 - 汇编代码会跳转到这里
#[no_mangle]
#[link_section = ".text.entry"]
fn _start() -> ! {
    unsafe {
        // 定义栈的边界
        let stack_bottom = STACK.as_ptr() as usize;
        let stack_top = stack_bottom + STACK_SIZE;
        
        // 关键：首先设置栈指针，这样我们才能执行Rust代码
        asm!(
            "mv sp, {0}",
            in(reg) stack_top,
        );
        
        // 关键：安全地清空BSS段，同时绕过栈区域。
        // 这是一个健壮内核的必要步骤，以防加载器未完成此工作。
        clear_bss(stack_bottom, stack_top);
        
        // 跳转到Rust主函数
        rust_main();
    }
}

/// Rust主函数 - 系统的真正入口点
#[no_mangle]
fn rust_main() -> ! {
    // 早期初始化阶段 - 不使用动态分配
    early_init();
    
    // 系统完整初始化 - 包括分配器
    init();
    
    // 验证系统状态
    verify_system_state();
    
    // 进入主循环
    main_loop();
}

/// 早期初始化 - 在分配器初始化前的基础设置
fn early_init() {
    use nt_rustos::{info_print, debug_print};
    
    info_print!("NT RustOS Early Initialization");
    info_print!("Version: 0.1.0");
    info_print!("Architecture: RISC-V 64-bit");
    info_print!("Stack size: {} KB", STACK_SIZE / 1024);
    
    // 显示启动信息
    debug_print!("Stack range: 0x{:x} - 0x{:x}", 
                unsafe { STACK.as_ptr() as usize },
                unsafe { STACK.as_ptr().add(STACK_SIZE) as usize });
    
    // 获取内核边界信息
    extern "C" {
        fn sbss();
        fn ebss();
        fn end();
    }
    
    let bss_start = unsafe { sbss as usize };
    let bss_end = unsafe { ebss as usize };
    let kernel_end = unsafe { end as usize };
    
    info_print!("Memory layout:");
    info_print!("  BSS: 0x{:x} - 0x{:x} ({} bytes)", 
               bss_start, bss_end, bss_end - bss_start);
    info_print!("  Kernel end: 0x{:x}", kernel_end);
    
    debug_print!("Early initialization completed");
}

/// 验证系统状态
fn verify_system_state() {
    use nt_rustos::{info_print, warn_print, error_print, init::alloc};
    
    info_print!("Verifying system state...");
    
    // 检查分配器状态
    if !alloc::is_initialized() {
        error_print!("FATAL: Allocator not initialized!");
        panic!("System verification failed");
    }
    
    if !alloc::is_enabled() {
        warn_print!("Warning: Allocator is disabled");
    }
    
    // 获取内存信息
    let memory_info = get_memory_info();
    memory_info.print();
    
    if !memory_info.is_healthy() {
        warn_print!("Memory system health issues detected");
    }
    
    // 执行完整性检查
    match alloc::integrity_check() {
        Ok(_) => { info_print!("Allocator integrity: OK"); }
        Err(e) => {
            error_print!("Allocator integrity check failed: {:?}", e);
            panic!("System verification failed");
        }
    }
    
    // 执行健康检查
    if let Some(health) = alloc::health_check() {
        if health.is_healthy() {
            info_print!("Allocator health: GOOD");
        } else {
            warn_print!("Allocator health issues detected");
            health.print_report();
        }
    }
    
    // 测试基本分配功能
    test_basic_allocation();
    
    info_print!("System verification completed successfully");
}

/// 测试基本分配功能
fn test_basic_allocation() {
    use nt_rustos::{info_print, error_print, init::alloc, Vec, String};
    
    info_print!("Testing basic allocation functionality...");
    
    // 测试原始分配
    if let Some(ptr) = alloc::alloc(1024) {
        // 写入测试数据
        unsafe {
            core::ptr::write_bytes(ptr, 0xAA, 1024);
            
            // 验证数据
            for i in 0..1024 {
                if core::ptr::read((ptr as *const u8).add(i)) != 0xAA {
                    error_print!("Data verification failed at offset {}", i);
                    alloc::dealloc(ptr);
                    panic!("Basic allocation test failed");
                }
            }
        }
        
        alloc::dealloc(ptr);
        info_print!("Raw allocation test: PASSED");
    } else {
        error_print!("Raw allocation test: FAILED");
        panic!("Basic allocation test failed");
    }
    
    // 测试Vec
    let mut test_vec = Vec::new();
    for i in 0..10 {
        test_vec.push(i * i);
    }
    
    if test_vec.len() == 10 && test_vec[5] == 25 {
        info_print!("Vec allocation test: PASSED");
    } else {
        error_print!("Vec allocation test: FAILED");
        panic!("Basic allocation test failed");
    }
    
    // 测试String
    let mut test_string = String::from("System");
    test_string.push_str(" Ready!");
    
    if test_string == "System Ready!" {
        info_print!("String allocation test: PASSED");
    } else {
        error_print!("String allocation test: FAILED");
        panic!("Basic allocation test failed");
    }
    
    info_print!("All basic allocation tests passed");
}