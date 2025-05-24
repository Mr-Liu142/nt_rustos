// nt_rustos/src/lib.rs

#![no_std]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

// 导入alloc crate以支持动态数据结构
extern crate alloc;

// 重新导出常用的alloc类型和宏
pub use alloc::vec::Vec;
pub use alloc::string::String;
pub use alloc::format; // 确保 format 宏可用
pub use alloc::vec;    // 确保 vec! 宏可用
pub use alloc::boxed::Box; // 确保 Box 可用

// 声明内核模块
pub mod console;
pub mod util;
pub mod init;
pub mod test;
pub mod trap; // 新增：声明 trap 子系统模块

use core::panic::PanicInfo;
use core::arch::asm;

// 设置全局分配器
#[global_allocator]
static GLOBAL_ALLOCATOR: init::alloc::global::EarlyGlobalAllocator = init::alloc::global::GLOBAL_EARLY_ALLOCATOR;

/// 启动栈大小 (16KB)
pub const STACK_SIZE: usize = 4096 * 4;

/// Panic处理器 - 当发生panic时调用
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // 尝试禁用中断，防止嵌套Panic或进一步错误
    unsafe { asm!("csrci sstatus, 1 << 1") };

    error_print!("KERNEL PANIC!");

    if let Some(location) = info.location() {
        error_print!("  Location: {}:{}", location.file(), location.line());
    }

    if let Some(message) = info.message() {
        error_print!("  Message: {}", message);
    } else {
        error_print!("  No panic message available.");
    }

    // 如果错误处理系统（特别是ErrorManager的panic_mode）已经初始化，则利用它
    // 这需要trap系统已经初始化
    if trap::infrastructure::di::is_initialized() {
        trap::infrastructure::di::with_trap_system(|ts| {
            if !ts.error_manager().is_panic_mode() {
                 ts.error_manager().enter_panic_mode(); // 进入Panic模式
            }
            // 可以在这里记录Panic到ErrorLog，如果ErrorManager支持
        });
    }


    // 如果分配器已初始化，打印内存状态
    if init::alloc::is_initialized() {
        warn_print!("Memory state at panic:");
        if let Some((total, used, free)) = init::alloc::usage_summary() {
            error_print!("  Total: {} KB, Used: {} KB, Free: {} KB",
                        total / 1024, used / 1024, free / 1024);
        }

        if let Some(stats) = init::alloc::stats() {
            error_print!("  Allocations: {}, Deallocations: {}",
                        stats.total_allocs, stats.total_frees);
            if stats.total_size > 0 { // 避免除以零
                error_print!("  Usage: {}%, Fragmentation: {}%",
                            stats.usage_percent(), stats.fragmentation_estimate());
            }
        }
    } else {
        error_print!("  Allocator not initialized. Cannot report memory state.");
    }

    error_print!("System halted.");
    // 无限循环，停止系统
    loop {
        unsafe {
            asm!("wfi"); // 等待中断，降低功耗
        }
    }
}

/// 安全地清空BSS段，但跳过指定的栈区域
pub unsafe fn clear_bss(stack_bottom: usize, stack_top: usize) {
    extern "C" {
        fn sbss();
        fn ebss();
    }

    let bss_start = sbss as usize;
    let bss_end = ebss as usize;

    // 清理从 BSS 开始到栈底的区域 (如果栈在BSS之前或部分重叠则跳过)
    if bss_start < stack_bottom {
        for addr in bss_start..core::cmp::min(bss_end, stack_bottom) {
            core::ptr::write_volatile(addr as *mut u8, 0);
        }
    }

    // 清理从栈顶到 BSS 结束的区域 (如果栈在BSS之后或部分重叠则跳过)
    if stack_top < bss_end {
        for addr in core::cmp::max(bss_start, stack_top)..bss_end {
            core::ptr::write_volatile(addr as *mut u8, 0);
        }
    }
}


/// 系统初始化
pub fn init() {
    info_print!("NT RustOS Initializing...");

    // 1. 初始化早期分配器 (必须首先完成)
    extern "C" {
        fn end(); // 链接器提供的内核结束地址
    }

    let heap_start = unsafe { end as usize };
    let heap_start_aligned = (heap_start + 0xF) & !0xF; // 16字节对齐
    let heap_size = 2 * 1024 * 1024; // 2MB

    match init::alloc::init(heap_start_aligned, heap_size) {
        Ok(_) => {
            info_print!("Early Allocator initialized at 0x{:x} (Size: {} KB).", heap_start_aligned, heap_size / 1024);
            if let Some(stats) = init::alloc::stats() {
                info_print!("  Initial Heap: Total: {} KB, Free: {} KB, Overhead: {} bytes",
                            stats.total_size / 1024,
                            stats.free_size / 1024,
                            stats.total_size - stats.free_size);
            }
        }
        Err(e) => {
            // Panic时控制台可能还未完全可用，尝试基础输出
            crate::console::print_str("FATAL: Failed to initialize early allocator: ");
            // 无法使用复杂的打印，因为分配器失败
            // crate::console::print_str(format!("{:?}", e).as_str()); // format! 需要分配
            crate::console::print_str("Halting.\n");
            loop { unsafe { asm!("wfi"); } } // 系统无法继续
        }
    }

    // 2. 初始化 Trap 子系统 (依赖分配器)
    // 使用 Direct 模式，因为 Vectored 模式需要更复杂的硬件支持和设置
    trap::init(trap::TrapMode::Direct);
    info_print!("Trap Subsystem initialized.");

    // 3. 测试动态数据结构 (依赖分配器和trap系统错误处理)
    test_dynamic_structures();

    // 4. 显示SBI系统信息 (可选，但有助于调试)
    util::sbi::info::print_sbi_info();

    info_print!("System Core Initialization Completed.");
}

/// 测试动态数据结构支持
fn test_dynamic_structures() {
    info_print!("Testing dynamic data structures...");

    let mut test_vec = Vec::new();
    for i in 0..10 {
        test_vec.push(i);
    }
    if test_vec.len() == 10 && test_vec[9] == 9 {
        debug_print!("  Vec test: PASSED ({} elements)", test_vec.len());
    } else {
        error_print!("  Vec test: FAILED");
    }


    let mut test_string = String::from("Hello");
    test_string.push_str(", RustOS!");
    if test_string == "Hello, RustOS!" {
        debug_print!("  String test: PASSED ('{}')", test_string);
    } else {
        error_print!("  String test: FAILED");
    }

    if let Some(boxed_value) = init::alloc::global::advanced::EarlyBox::new(42u32) {
        if *boxed_value == 42 {
            debug_print!("  EarlyBox test: PASSED (value = {})", *boxed_value);
        } else {
            error_print!("  EarlyBox test: FAILED");
        }
    } else {
        error_print!("  EarlyBox allocation: FAILED");
    }

    info_print!("Dynamic structures test completed.");
}

/// 主循环 - 系统的核心循环
pub fn main_loop() -> ! {
    info_print!("Entering main operating loop...");

    // 运行所有测试
    info_print!("Running comprehensive test suites...");
    test::run_all_tests();
    info_print!("All test suites completed.");

    // 打印最终内存状态
    init::alloc::print_status();

    info_print!("System ready. Entering idle loop.");
    loop {
        unsafe {
            // 等待中断，如果没有中断发生，wfi 将使处理器进入低功耗状态
            // 直到下一个中断到达。
            asm!("wfi");
        }
        // 当中断发生并处理完毕后，会从这里继续执行。
        // 在一个更复杂的内核中，这里可能会检查调度队列等。
    }
}

/// 系统关闭
pub fn shutdown() -> ! {
    info_print!("System Shutting Down...");

    if init::alloc::is_initialized() {
        if let Some(handover) = init::alloc::prepare_handover() {
            info_print!("Final system state prepared for handover.");
            handover.print_summary();
        }
        if let Err(e) = init::alloc::freeze() {
            error_print!("Failed to freeze allocator: {:?}", e);
        }
        init::alloc::print_status();
    }

    info_print!("Shutdown sequence completed. Calling SBI shutdown.");
    util::sbi::system::shutdown();
}

/// 系统重启
pub fn reboot() -> ! {
    info_print!("System Rebooting...");
    if init::alloc::is_initialized() {
        if let Err(e) = init::alloc::freeze() {
            warn_print!("Failed to freeze allocator before reboot: {:?}", e);
        }
    }
    info_print!("Reboot sequence initiated. Calling SBI reboot.");
    util::sbi::system::reboot();
}

/// 获取系统内存信息摘要
pub fn get_memory_info() -> MemoryInfo {
    let (total_size, used_size, free_size) = init::alloc::usage_summary().unwrap_or((0, 0, 0));
    let (alloc_count, dealloc_count) = if let Some(stats) = init::alloc::stats() {
        (stats.total_allocs, stats.total_frees)
    } else {
        (0, 0)
    };

    MemoryInfo {
        total_size,
        used_size,
        free_size,
        allocator_initialized: init::alloc::is_initialized(),
        allocator_enabled: init::alloc::is_enabled(),
        allocation_count: alloc_count,
        deallocation_count: dealloc_count,
    }
}

/// 系统内存信息
#[derive(Debug, Clone)]
pub struct MemoryInfo {
    pub total_size: usize,
    pub used_size: usize,
    pub free_size: usize,
    pub allocator_initialized: bool,
    pub allocator_enabled: bool,
    pub allocation_count: u64,
    pub deallocation_count: u64,
}

impl MemoryInfo {
    /// 打印内存信息
    pub fn print(&self) {
        println!("=== System Memory Information ===");
        println!("  Total: {} KB", self.total_size / 1024);
        let usage_percent = if self.total_size > 0 { self.used_size * 100 / self.total_size } else { 0 };
        println!("  Used:  {} KB ({}%)", self.used_size / 1024, usage_percent);
        println!("  Free:  {} KB", self.free_size / 1024);
        println!("  Allocator: Initialized ({}), Enabled ({})",
                 self.allocator_initialized, self.allocator_enabled);
        println!("  Operations: {} allocs, {} deallocs",
                 self.allocation_count, self.deallocation_count);
        println!("=================================");
    }

    /// 获取使用率百分比
    pub fn usage_percent(&self) -> u8 {
        if self.total_size == 0 {
            0
        } else {
            ((self.used_size * 100) / self.total_size) as u8
        }
    }

    /// 检查内存是否健康
    pub fn is_healthy(&self) -> bool {
        self.allocator_initialized &&
        self.usage_percent() < 95 && // 允许更高的使用率，90%可能过于保守
        self.allocation_count >= self.deallocation_count // 基本的泄漏检查
    }
}