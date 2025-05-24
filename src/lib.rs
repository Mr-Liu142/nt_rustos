#![no_std]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

// 导入alloc crate以支持动态数据结构
extern crate alloc;

// 重新导出常用的alloc类型和宏
pub use alloc::vec::Vec;
pub use alloc::string::String;
pub use alloc::format;
pub use alloc::vec;

// 导出核心模块
pub mod console;
pub mod util;
pub mod init;
pub mod test;

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
    // 输出panic信息
    error_print!("Kernel panic!");
    
    if let Some(location) = info.location() {
        error_print!("Location: {}:{}", location.file(), location.line());
    }
    
    if let Some(message) = info.message() {
        error_print!("Message: {}", message);
    }
    
    // 如果分配器已初始化，打印内存状态
    if init::alloc::is_initialized() {
        warn_print!("Memory state at panic:");
        if let Some((total, used, free)) = init::alloc::usage_summary() {
            error_print!("  Total: {} KB, Used: {} KB, Free: {} KB", 
                        total / 1024, used / 1024, free / 1024);
        }
        
        // 尝试获取详细统计
        if let Some(stats) = init::alloc::stats() {
            error_print!("  Allocations: {}, Deallocations: {}", 
                        stats.total_allocs, stats.total_frees);
            error_print!("  Usage: {}%, Fragmentation: {}%", 
                        stats.usage_percent(), stats.fragmentation_estimate());
        }
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
    
    // 初始化早期分配器
    // 假设在BSS段之后有2MB空间可用于早期堆
    extern "C" {
        fn end(); // 链接器提供的内核结束地址
    }
    
    let heap_start = unsafe { end as usize };
    let heap_start_aligned = (heap_start + 0xF) & !0xF; // 16字节对齐
    let heap_size = 2 * 1024 * 1024; // 2MB - 增加堆大小以支持更多动态分配
    
    match init::alloc::init(heap_start_aligned, heap_size) {
        Ok(_) => {
            info_print!("Early allocator initialized at 0x{:x}", heap_start_aligned);
            
            // 打印早期分配器详细状态
            if let Some(stats) = init::alloc::stats() {
                info_print!("Early heap initialized successfully:");
                info_print!("  Total: {} KB", stats.total_size / 1024);
                info_print!("  Available: {} KB", stats.free_size / 1024);
                info_print!("  Overhead: {} bytes", stats.total_size - stats.free_size);
            }
            
            // 执行分配器健康检查
            if let Some(health) = init::alloc::health_check() {
                if health.is_healthy() {
                    info_print!("Allocator health check: PASSED");
                } else {
                    warn_print!("Allocator health check: ISSUES DETECTED");
                    health.print_report();
                }
            }
            
            // 测试动态数据结构支持
            test_dynamic_structures();
        }
        Err(e) => {
            error_print!("Failed to initialize early allocator: {:?}", e);
            // 早期分配器初始化失败是致命错误
            panic!("Cannot continue without early allocator");
        }
    }
    
    // 显示SBI系统信息
    util::sbi::info::print_sbi_info();
    
    info_print!("System initialization completed");
}

/// 测试动态数据结构支持
fn test_dynamic_structures() {
    info_print!("Testing dynamic data structures...");
    
    // 测试基本Vec操作
    let mut test_vec = Vec::new();
    for i in 0..10 {
        test_vec.push(i);
    }
    
    info_print!("Vec test: created vector with {} elements", test_vec.len());
    
    // 测试String操作
    let mut test_string = String::from("Hello");
    test_string.push_str(", World!");
    info_print!("String test: '{}'", test_string);
    
    // 测试自定义智能指针
    if let Some(boxed_value) = init::alloc::global::advanced::EarlyBox::new(42u32) {
        info_print!("EarlyBox test: value = {}", *boxed_value);
        
        // 设置分配用途
        if let Err(e) = boxed_value.set_purpose(init::alloc::AllocPurpose::Testing) {
            warn_print!("Failed to set purpose: {:?}", e);
        }
    }
    
    // 测试自定义Vec
    let mut custom_vec = init::alloc::global::advanced::EarlyVec::new();
    for i in 0..5 {
        custom_vec.push(i * 10);
    }
    info_print!("EarlyVec test: {} elements", custom_vec.len());
    
    info_print!("Dynamic structures test completed");
    
    // 打印测试后的内存状态
    if let Some(stats) = init::alloc::stats() {
        info_print!("Memory after dynamic tests:");
        info_print!("  Used: {} KB ({}%)", stats.used_size / 1024, stats.usage_percent());
        info_print!("  Allocations: {}", stats.total_allocs);
    }
}

/// 主循环 - 系统的核心循环
pub fn main_loop() -> ! {
    info_print!("Entering main loop");
    
    // 创建内存快照
    let snapshot_before = init::alloc::create_snapshot();
    
    // 运行所有测试
    info_print!("Running comprehensive tests...");
    test::run_all_tests();
    info_print!("All tests completed");
    
    // 创建测试后的快照并比较
    if let (Some(before), Some(after)) = (snapshot_before, init::alloc::create_snapshot()) {
        info_print!("Memory usage comparison:");
        let comparison = before.compare(&after);
        comparison.print();
    }
    
    // 执行维护任务
    info_print!("Running allocator maintenance...");
    if let Err(e) = init::alloc::maintenance() {
        warn_print!("Maintenance failed: {:?}", e);
    }
    
    // 打印最终内存状态
    init::alloc::print_status();
    
    // 演示高级内存管理功能
    demonstrate_advanced_features();
    
    // 主循环 - 目前只是等待中断
    info_print!("System ready, entering idle loop");
    loop {
        unsafe {
            asm!("wfi"); // 等待中断，降低功耗
        }
    }
}

/// 演示高级内存管理功能
fn demonstrate_advanced_features() {
    info_print!("Demonstrating advanced memory management features...");
    
    // 演示不同用途的内存分配
    demonstrate_purpose_allocation();
    
    // 演示内存快照和比较
    demonstrate_snapshots();
    
    // 演示紧急回收
    demonstrate_emergency_reclaim();
    
    info_print!("Advanced features demonstration completed");
}

/// 演示按用途分配内存
fn demonstrate_purpose_allocation() {
    use init::alloc::AllocPurpose;
    
    info_print!("Testing purpose-based allocation...");
    
    // 分配不同用途的内存块
    let purposes = [
        (AllocPurpose::KernelStack, 4096),
        (AllocPurpose::PageTable, 8192),
        (AllocPurpose::NetworkBuffer, 1024),
        (AllocPurpose::TempBuffer, 512),
        (AllocPurpose::CacheBuffer, 2048),
    ];
    
    let mut allocated_ptrs = Vec::new();
    
    for (purpose, size) in purposes.iter() {
        if let Some(ptr) = alloc_with_purpose!(*size, *purpose) {
            allocated_ptrs.push((ptr, *size));
            debug_print!("Allocated {} bytes for {}", size, purpose.description());
        } else {
            warn_print!("Failed to allocate for {}", purpose.description());
        }
    }
    
    // 打印分配后的接管信息
    if let Some(handover) = init::alloc::prepare_handover() {
        info_print!("Purpose allocation results:");
        let groups = handover.group_by_purpose();
        for (purpose, count, size) in &groups {
            if *count > 0 {
                info_print!("  {}: {} blocks, {} bytes", 
                           purpose.description(), count, size);
            }
        }
        
        info_print!("Reclaimable: {} bytes", handover.reclaimable_size());
        info_print!("Critical: {} bytes", handover.critical_size());
    }
    
    // 清理分配的内存
    for (ptr, size) in allocated_ptrs {
        if let Err(e) = init::alloc::dealloc_safe(ptr, size) {
            warn_print!("Failed to deallocate: {:?}", e);
        }
    }
}

/// 演示内存快照功能
fn demonstrate_snapshots() {
    info_print!("Testing memory snapshot functionality...");
    
    // 创建第一个快照
    let snapshot1 = init::alloc::create_snapshot();
    
    // 进行一些分配
    let mut temp_allocations = Vec::new();
    for i in 0..5 {
        if let Some(ptr) = init::alloc::alloc(1024 * (i + 1)) {
            temp_allocations.push((ptr, 1024 * (i + 1)));
        }
    }
    
    // 创建第二个快照
    let snapshot2 = init::alloc::create_snapshot();
    
    // 比较快照
    if let (Some(s1), Some(s2)) = (snapshot1, snapshot2) {
        info_print!("Snapshot comparison results:");
        let comparison = s1.compare(&s2);
        comparison.print();
    }
    
    // 清理临时分配
    for (ptr, size) in temp_allocations {
        if let Err(e) = init::alloc::dealloc_safe(ptr, size) {
            warn_print!("Failed to cleanup temp allocation: {:?}", e);
        }
    }
}

/// 演示紧急内存回收
fn demonstrate_emergency_reclaim() {
    info_print!("Testing emergency memory reclaim...");
    
    // 创建一些临时分配
    let mut temp_ptrs = Vec::new();
    for _ in 0..3 {
        if let Some(ptr) = alloc_with_purpose!(2048, init::alloc::AllocPurpose::TempBuffer) {
            temp_ptrs.push(ptr);
        }
    }
    
    // 执行紧急回收
    let reclaimed = init::alloc::emergency_reclaim();
    info_print!("Emergency reclaim identified {} bytes", reclaimed);
    
    // 在实际系统中，这些临时缓冲区可能会被自动回收
    // 这里我们手动清理
    for ptr in temp_ptrs {
        if let Err(e) = init::alloc::dealloc_safe(ptr, 2048) {
            warn_print!("Failed to cleanup temp buffer: {:?}", e);
        }
    }
}

/// 系统关闭
pub fn shutdown() -> ! {
    info_print!("System shutting down...");
    
    // 准备接管信息（用于调试或重启后恢复）
    if let Some(handover) = init::alloc::prepare_handover() {
        info_print!("Final system state prepared for handover");
        handover.print_summary();
    }
    
    // 冻结分配器
    if let Err(e) = init::alloc::freeze() {
        error_print!("Failed to freeze allocator: {:?}", e);
    }
    
    // 最终内存状态报告
    init::alloc::print_status();
    
    info_print!("System shutdown sequence completed");
    util::sbi::system::shutdown();
}

/// 系统重启
pub fn reboot() -> ! {
    info_print!("System rebooting...");
    
    // 执行重启前的清理
    if init::alloc::is_initialized() {
        warn_print!("Performing pre-reboot cleanup...");
        
        // 执行最后的维护
        if let Err(e) = init::alloc::maintenance() {
            warn_print!("Pre-reboot maintenance failed: {:?}", e);
        }
        
        // 冻结分配器
        if let Err(e) = init::alloc::freeze() {
            warn_print!("Failed to freeze allocator before reboot: {:?}", e);
        }
    }
    
    info_print!("Reboot sequence initiated");
    util::sbi::system::reboot();
}

/// 获取系统内存信息摘要
pub fn get_memory_info() -> MemoryInfo {
    MemoryInfo {
        total_size: if let Some((total, _, _)) = init::alloc::usage_summary() {
            total
        } else {
            0
        },
        used_size: if let Some((_, used, _)) = init::alloc::usage_summary() {
            used
        } else {
            0
        },
        free_size: if let Some((_, _, free)) = init::alloc::usage_summary() {
            free
        } else {
            0
        },
        allocator_initialized: init::alloc::is_initialized(),
        allocator_enabled: init::alloc::is_enabled(),
        allocation_count: if let Some(stats) = init::alloc::stats() {
            stats.total_allocs
        } else {
            0
        },
        deallocation_count: if let Some(stats) = init::alloc::stats() {
            stats.total_frees
        } else {
            0
        },
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
        println!("Total: {} KB", self.total_size / 1024);
        println!("Used:  {} KB ({}%)", 
                 self.used_size / 1024,
                 if self.total_size > 0 { self.used_size * 100 / self.total_size } else { 0 });
        println!("Free:  {} KB", self.free_size / 1024);
        println!("Allocator: {} ({})", 
                 if self.allocator_initialized { "Initialized" } else { "Not Initialized" },
                 if self.allocator_enabled { "Enabled" } else { "Disabled" });
        println!("Operations: {} allocs, {} deallocs", 
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
        self.usage_percent() < 90 &&
        self.allocation_count >= self.deallocation_count
    }
}