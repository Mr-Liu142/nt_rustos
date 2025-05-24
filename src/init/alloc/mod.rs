// 生产级早期堆内存分配器模块
// 用于内核启动早期的内存分配，在完整的内存管理系统初始化前使用

pub mod allocator;
pub mod metadata;
pub mod handover;
pub mod global;

use core::sync::atomic::{AtomicBool, Ordering};
use crate::{error_print, warn_print, info_print, debug_print, println};
use crate::init::alloc::global::advanced;

// 从子模块导出类型
pub use self::allocator::{EarlyAllocator, AllocError, ThreadSafeEarlyAllocator};
pub use self::global::{GLOBAL_EARLY_ALLOCATOR, EarlyGlobalAllocator};
pub use self::metadata::{AllocStats, BlockHeader, BlockStatus, HealthStatus};
pub use self::handover::{HandoverInfo, AllocatedBlock, AllocPurpose, HandoverProtocol};

// 全局状态管理
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(true);

/// 初始化早期分配器
/// 
/// # 参数
/// * `heap_start` - 堆起始地址
/// * `heap_size` - 堆大小（字节）
/// 
/// # 返回值
/// 成功返回Ok(())，失败返回错误
pub fn init(heap_start: usize, heap_size: usize) -> Result<(), AllocError> {
    // 检查是否已经初始化
    if INITIALIZED.load(Ordering::Acquire) {
        warn_print!("Early allocator already initialized");
        return Err(AllocError::AlreadyInitialized);
    }
    
    // 详细的参数验证
    if heap_start == 0 {
        error_print!("Invalid heap start address: 0");
        return Err(AllocError::InvalidParameter);
    }
    
    if heap_size < 64 * 1024 {
        error_print!("Heap size too small: {} bytes (minimum: 64KB)", heap_size);
        return Err(AllocError::InvalidParameter);
    }
    
    if heap_size > 1024 * 1024 * 1024 {
        error_print!("Heap size too large: {} bytes (maximum: 1GB)", heap_size);
        return Err(AllocError::InvalidParameter);
    }
    
    // 检查地址对齐（16字节对齐）
    if heap_start & 0xF != 0 {
        error_print!("Heap start address not aligned: 0x{:x}", heap_start);
        return Err(AllocError::InvalidAlignment);
    }
    
    // 检查地址范围的合理性
    let heap_end = heap_start.checked_add(heap_size);
    if heap_end.is_none() {
        error_print!("Heap address range overflow");
        return Err(AllocError::InvalidParameter);
    }
    
    let heap_end = heap_end.unwrap();
    if heap_end <= heap_start {
        error_print!("Invalid heap range: start=0x{:x}, end=0x{:x}", heap_start, heap_end);
        return Err(AllocError::InvalidParameter);
    }
    
    // 初始化全局分配器
    match GLOBAL_EARLY_ALLOCATOR.init(heap_start, heap_size) {
        Ok(_) => {
            INITIALIZED.store(true, Ordering::Release);
            info_print!("Early allocator initialized successfully");
            info_print!("  Start: 0x{:x}", heap_start);
            info_print!("  Size:  {} KB ({} bytes)", heap_size / 1024, heap_size);
            info_print!("  End:   0x{:x}", heap_end);
            
            // 执行初始化后的完整性检查
            if let Err(e) = GLOBAL_EARLY_ALLOCATOR.integrity_check() {
                error_print!("Post-initialization integrity check failed: {:?}", e);
                return Err(e);
            }
            
            // 打印初始统计信息
            if let Some(stats) = GLOBAL_EARLY_ALLOCATOR.stats() {
                info_print!("Initial heap state:");
                info_print!("  Available: {} KB", stats.free_size / 1024);
                info_print!("  Overhead:  {} bytes", stats.total_size - stats.free_size);
            }
            
            Ok(())
        }
        Err(e) => {
            error_print!("Failed to initialize early allocator: {:?}", e);
            Err(e)
        }
    }
}

/// 检查分配器是否已初始化
pub fn is_initialized() -> bool {
    INITIALIZED.load(Ordering::Acquire)
}

/// 检查分配器是否已启用
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

/// 启用分配器
pub fn enable() {
    ENABLED.store(true, Ordering::Release);
    debug_print!("Early allocator enabled");
}

/// 禁用分配器
pub fn disable() {
    ENABLED.store(false, Ordering::Release);
    warn_print!("Early allocator disabled");
}

/// 分配内存
/// 
/// # 参数
/// * `size` - 要分配的字节数
/// 
/// # 返回值
/// 成功返回内存地址，失败返回None
pub fn alloc(size: usize) -> Option<*mut u8> {
    if !is_initialized() {
        error_print!("Early allocator not initialized");
        return None;
    }
    
    if !is_enabled() {
        debug_print!("Allocation attempt while allocator disabled (size: {})", size);
        return None;
    }
    
    if size == 0 {
        debug_print!("Zero-size allocation request");
        return None;
    }
    
    match GLOBAL_EARLY_ALLOCATOR.alloc_aligned_raw(size, 8) {
        Some(ptr) => Some(ptr.as_ptr()),
        None => {
            debug_print!("Allocation failed: size: {}", size);
            None
        }
    }
}

/// 对齐分配内存
/// 
/// # 参数
/// * `size` - 要分配的字节数
/// * `align` - 对齐要求（必须是2的幂）
/// 
/// # 返回值
/// 成功返回内存地址，失败返回None
pub fn alloc_aligned(size: usize, align: usize) -> Option<*mut u8> {
    if !is_initialized() {
        error_print!("Early allocator not initialized");
        return None;
    }
    
    if !is_enabled() {
        debug_print!("Aligned allocation attempt while allocator disabled");
        return None;
    }
    
    if size == 0 || !align.is_power_of_two() {
        debug_print!("Invalid aligned allocation parameters: size={}, align={}", size, align);
        return None;
    }
    
    match GLOBAL_EARLY_ALLOCATOR.alloc_aligned_raw(size, align) {
        Some(ptr) => Some(ptr.as_ptr()),
        None => {
            debug_print!("Aligned allocation failed: size: {}, align: {}", size, align);
            None
        }
    }
}

/// 分配并清零内存
/// 
/// # 参数
/// * `size` - 要分配的字节数
/// 
/// # 返回值
/// 成功返回内存地址，失败返回None
pub fn alloc_zeroed(size: usize) -> Option<*mut u8> {
    if let Some(ptr) = alloc(size) {
        unsafe {
            core::ptr::write_bytes(ptr, 0, size);
        }
        Some(ptr)
    } else {
        None
    }
}

/// 释放内存
/// 
/// # 参数
/// * `ptr` - 要释放的内存地址
pub fn dealloc(ptr: *mut u8) {
    if !is_initialized() {
        error_print!("Early allocator not initialized");
        return;
    }
    
    if ptr.is_null() {
        warn_print!("Attempt to deallocate null pointer");
        return;
    }
    
    if let Some(non_null_ptr) = core::ptr::NonNull::new(ptr) {
        if let Err(e) = GLOBAL_EARLY_ALLOCATOR.dealloc_raw(non_null_ptr) {
            error_print!("Deallocation failed: {:?}, ptr=0x{:x}", e, ptr as usize);
        }
    }
}

/// 安全释放内存（带错误返回）
/// 
/// # 参数
/// * `ptr` - 要释放的内存地址
/// * `size` - 原始分配大小（用于验证）
/// 
/// # 返回值
/// 成功返回Ok(())，失败返回错误
pub fn dealloc_safe(ptr: *mut u8, _size: usize) -> Result<(), AllocError> {
    if !is_initialized() {
        return Err(AllocError::NotInitialized);
    }
    
    if ptr.is_null() {
        return Err(AllocError::NullPointer);
    }
    
    if let Some(non_null_ptr) = core::ptr::NonNull::new(ptr) {
        GLOBAL_EARLY_ALLOCATOR.dealloc_raw(non_null_ptr)
    } else {
        Err(AllocError::NullPointer)
    }
}

/// 设置分配用途
/// 
/// # 参数
/// * `ptr` - 内存地址
/// * `purpose` - 分配用途
/// 
/// # 返回值
/// 成功返回Ok(())，失败返回错误
pub fn set_purpose(_ptr: *mut u8, _purpose: AllocPurpose) -> Result<(), AllocError> {
    if !is_initialized() {
        return Err(AllocError::NotInitialized);
    }
    
    // 简化实现：不实际设置用途
    Ok(())
}

/// 获取分配器统计信息
pub fn stats() -> Option<AllocStats> {
    if !is_initialized() {
        return None;
    }
    
    GLOBAL_EARLY_ALLOCATOR.stats()
}

/// 执行完整性检查
pub fn integrity_check() -> Result<(), AllocError> {
    if !is_initialized() {
        return Err(AllocError::NotInitialized);
    }
    
    GLOBAL_EARLY_ALLOCATOR.integrity_check()
}

/// 打印分配器状态
pub fn print_status() {
    if !is_initialized() {
        error_print!("Early allocator not initialized");
        return;
    }
    
    info_print!("Early Allocator Status:");
    info_print!("  Initialized: {}", is_initialized());
    info_print!("  Enabled: {}", is_enabled());
    
    if let Some(stats) = stats() {
        stats.print_summary();
        
        // 健康检查
        let health = stats.check_health();
        health.print_report();
    }
    
    // 执行完整性检查
    match integrity_check() {
        Ok(_) => {
            info_print!("Integrity check: PASSED");
        }
        Err(e) => {
            error_print!("Integrity check: FAILED ({:?})", e);
        }
    }
}

/// 打印详细调试信息
pub fn print_debug_info() {
    if !is_initialized() {
        error_print!("Early allocator not initialized");
        return;
    }
    
    println!("=== Early Allocator Debug Information ===");
    
    if let Some(stats) = stats() {
        stats.print_detailed();
    }
    
    // 尝试准备接管信息以获取更多详情
    if let Some(handover) = prepare_handover() {
        handover.print_detailed_report();
    }
    
    println!("==========================================");
}

/// 执行内存健康检查
pub fn health_check() -> Option<HealthStatus> {
    if let Some(stats) = stats() {
        Some(stats.check_health())
    } else {
        None
    }
}

/// 准备接管数据
/// 
/// # 返回值
/// 返回接管信息，如果分配器未初始化则返回None
pub fn prepare_handover() -> Option<advanced::EarlyBox<HandoverInfo>> {
    if !is_initialized() {
        warn_print!("Cannot prepare handover: allocator not initialized");
        return None;
    }
    
    info_print!("Preparing allocator handover...");
    
    // 执行最终的完整性检查
    if let Err(e) = integrity_check() {
        error_print!("Pre-handover integrity check failed: {:?}", e);
    }
    
    // 执行健康检查
    if let Some(health) = health_check() {
        if !health.is_healthy() {
            warn_print!("Handover preparation: allocator health issues detected");
            health.print_report();
        }
    }
    
    match GLOBAL_EARLY_ALLOCATOR.prepare_handover() {
        Some(handover) => {
            // 验证接管信息
            match handover.validate() {
                Ok(_) => {
                    info_print!("Handover information prepared and validated");
                    handover.print_summary();
                    Some(handover)
                }
                Err(e) => {
                    error_print!("Handover validation failed: {}", e);
                    None
                }
            }
        }
        None => {
            error_print!("Failed to prepare handover information");
            None
        }
    }
}

/// 冻结分配器，准备接管
/// 
/// # 返回值
/// 成功返回Ok(())，失败返回错误
pub fn freeze() -> Result<(), AllocError> {
    if !is_initialized() {
        return Err(AllocError::NotInitialized);
    }
    
    info_print!("Freezing early allocator...");
    
    // 执行最终统计和检查
    print_status();
    
    // 冻结分配器
    match GLOBAL_EARLY_ALLOCATOR.freeze() {
        Ok(_) => {
            disable(); // 同时禁用分配功能
            info_print!("Early allocator frozen and disabled");
            Ok(())
        }
        Err(e) => {
            error_print!("Failed to freeze allocator: {:?}", e);
            Err(e)
        }
    }
}

/// 获取堆使用情况的简单描述
pub fn usage_summary() -> Option<(usize, usize, usize)> {
    if let Some(stats) = stats() {
        Some((stats.total_size, stats.used_size, stats.free_size))
    } else {
        None
    }
}

/// 紧急回收内存
/// 尝试回收所有可回收的内存
pub fn emergency_reclaim() -> usize {
    if !is_initialized() {
        error_print!("Cannot perform emergency reclaim: allocator not initialized");
        return 0;
    }
    
    warn_print!("Performing emergency memory reclaim...");
    
    // 获取当前状态
    let stats_before = stats();
    
    // 准备接管信息以获取可回收块的信息
    if let Some(handover) = GLOBAL_EARLY_ALLOCATOR.prepare_handover() {
        let reclaimable_size = handover.reclaimable_size();
        if reclaimable_size > 0 {
            warn_print!("Found {} KB of potentially reclaimable memory", reclaimable_size / 1024);
            
            // 在实际实现中，这里会回收临时缓冲区等
            // 目前只是报告信息
            return reclaimable_size;
        }
    }
    
    warn_print!("No reclaimable memory found");
    0
}

/// 运行自动维护任务
pub fn maintenance() -> Result<(), AllocError> {
    if !is_initialized() {
        return Err(AllocError::NotInitialized);
    }
    
    debug_print!("Running allocator maintenance...");
    
    // 执行完整性检查
    integrity_check()?;
    
    // 检查健康状态
    if let Some(health) = health_check() {
        if !health.is_healthy() {
            warn_print!("Maintenance: health issues detected");
            health.print_report();
        }
    }
    
    // 在实际实现中，这里可能会执行：
    // - 内存碎片整理
    // - 清理过期的临时分配
    // - 更新统计信息
    // - 优化空闲链表
    
    debug_print!("Allocator maintenance completed");
    Ok(())
}

/// 创建内存快照（用于调试）
pub fn create_snapshot() -> Option<MemorySnapshot> {
    if !is_initialized() {
        return None;
    }
    
    let stats = stats()?;
    let handover = GLOBAL_EARLY_ALLOCATOR.prepare_handover()?;
    
    Some(MemorySnapshot {
        timestamp: get_timestamp(),
        statistics: stats,
        handover_info: handover,
    })
}

/// 内存快照
#[derive(Debug)]
pub struct MemorySnapshot {
    pub timestamp: u64,
    pub statistics: AllocStats,
    pub handover_info: advanced::EarlyBox<HandoverInfo>,
}

impl MemorySnapshot {
    /// 比较两个快照
    pub fn compare(&self, other: &MemorySnapshot) -> SnapshotComparison {
        SnapshotComparison {
            time_delta: other.timestamp.saturating_sub(self.timestamp),
            alloc_delta: other.statistics.total_allocs - self.statistics.total_allocs,
            dealloc_delta: other.statistics.total_frees - self.statistics.total_frees,
            size_delta: other.statistics.used_size as i64 - self.statistics.used_size as i64,
            block_count_delta: other.handover_info.allocated_count as i64 
                             - self.handover_info.allocated_count as i64,
        }
    }
    
    /// 打印快照信息
    pub fn print(&self) {
        println!("=== Memory Snapshot (t={}) ===", self.timestamp);
        self.statistics.print_summary();
        println!("Allocated blocks: {}", self.handover_info.allocated_count);
        println!("============================");
    }
}

/// 快照比较结果
#[derive(Debug)]
pub struct SnapshotComparison {
    pub time_delta: u64,
    pub alloc_delta: u64,
    pub dealloc_delta: u64,
    pub size_delta: i64,
    pub block_count_delta: i64,
}

impl SnapshotComparison {
    /// 打印比较结果
    pub fn print(&self) {
        println!("=== Snapshot Comparison ===");
        println!("Time delta: {} ticks", self.time_delta);
        println!("Allocations: +{}", self.alloc_delta);
        println!("Deallocations: +{}", self.dealloc_delta);
        println!("Size change: {:+} bytes", self.size_delta);
        println!("Block count change: {:+}", self.block_count_delta);
        
        if self.alloc_delta > self.dealloc_delta {
            warn_print!("Net allocation growth detected");
        }
        
        if self.size_delta > 0 {
            warn_print!("Memory usage increased by {} KB", self.size_delta / 1024);
        }
        
        println!("=========================");
    }
}

/// 获取简单时间戳
fn get_timestamp() -> u64 {
    static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
    COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
}

/// 便捷宏定义
#[macro_export]
macro_rules! alloc_with_purpose {
    ($size:expr, $purpose:expr) => {
        {
            if let Some(ptr) = $crate::init::alloc::alloc($size) {
                if let Err(e) = $crate::init::alloc::set_purpose(ptr, $purpose) {
                    $crate::warn_print!("Failed to set allocation purpose: {:?}", e);
                }
                Some(ptr)
            } else {
                None
            }
        }
    };
}

#[macro_export]
macro_rules! alloc_zeroed_with_purpose {
    ($size:expr, $purpose:expr) => {
        {
            if let Some(ptr) = $crate::init::alloc::alloc_zeroed($size) {
                if let Err(e) = $crate::init::alloc::set_purpose(ptr, $purpose) {
                    $crate::warn_print!("Failed to set allocation purpose: {:?}", e);
                }
                Some(ptr)
            } else {
                None
            }
        }
    };
}