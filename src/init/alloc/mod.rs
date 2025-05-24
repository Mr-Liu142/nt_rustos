// 早期堆内存分配器模块
// 用于内核启动早期的内存分配，在完整的内存管理系统初始化前使用

pub mod allocator;
pub mod metadata;
pub mod handover;

use core::sync::atomic::{AtomicBool, Ordering};
use crate::{error_print, warn_print, info_print};

// 从子模块导出类型
pub use self::allocator::{EarlyAllocator, AllocError};
pub use self::metadata::{AllocStats, BlockHeader, BlockStatus};
pub use self::handover::{HandoverInfo, AllocatedBlock, AllocPurpose};

// 全局早期分配器实例
static mut EARLY_ALLOCATOR: Option<EarlyAllocator> = None;
static INITIALIZED: AtomicBool = AtomicBool::new(false);

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
    
    // 检查参数有效性
    if heap_start == 0 {
        error_print!("Invalid heap start address: 0");
        return Err(AllocError::InvalidParameter);
    }
    
    if heap_size < 4096 {
        error_print!("Heap size too small: {} bytes", heap_size);
        return Err(AllocError::InvalidParameter);
    }
    
    // 检查地址对齐（16字节对齐）
    if heap_start & 0xF != 0 {
        error_print!("Heap start address not aligned: 0x{:x}", heap_start);
        return Err(AllocError::InvalidAlignment);
    }
    
    // 创建分配器实例
    unsafe {
        match EarlyAllocator::new(heap_start, heap_size) {
            Ok(allocator) => {
                EARLY_ALLOCATOR = Some(allocator);
                INITIALIZED.store(true, Ordering::Release);
                info_print!("Early allocator initialized: start=0x{:x}, size={}KB", 
                           heap_start, heap_size / 1024);
                Ok(())
            }
            Err(e) => {
                error_print!("Failed to create early allocator: {:?}", e);
                Err(e)
            }
        }
    }
}

/// 检查分配器是否已初始化
pub fn is_initialized() -> bool {
    INITIALIZED.load(Ordering::Acquire)
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
    
    unsafe {
        if let Some(ref mut allocator) = EARLY_ALLOCATOR {
            allocator.alloc(size)
        } else {
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
    
    // 检查对齐参数是否为2的幂
    if align == 0 || (align & (align - 1)) != 0 {
        error_print!("Invalid alignment: {}", align);
        return None;
    }
    
    unsafe {
        if let Some(ref mut allocator) = EARLY_ALLOCATOR {
            allocator.alloc_aligned(size, align)
        } else {
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
            // 清零内存
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
    
    unsafe {
        if let Some(ref mut allocator) = EARLY_ALLOCATOR {
            if let Err(e) = allocator.dealloc(ptr) {
                error_print!("Deallocation failed: {:?}", e);
            }
        }
    }
}

/// 安全释放内存（带错误返回）
/// 
/// # 参数
/// * `ptr` - 要释放的内存地址
/// 
/// # 返回值
/// 成功返回Ok(())，失败返回错误
pub fn dealloc_safe(ptr: *mut u8) -> Result<(), AllocError> {
    if !is_initialized() {
        return Err(AllocError::NotInitialized);
    }
    
    if ptr.is_null() {
        return Err(AllocError::NullPointer);
    }
    
    unsafe {
        if let Some(ref mut allocator) = EARLY_ALLOCATOR {
            allocator.dealloc(ptr)
        } else {
            Err(AllocError::NotInitialized)
        }
    }
}

/// 获取分配器统计信息
pub fn stats() -> Option<AllocStats> {
    if !is_initialized() {
        return None;
    }
    
    unsafe {
        if let Some(ref allocator) = EARLY_ALLOCATOR {
            Some(allocator.stats())
        } else {
            None
        }
    }
}

/// 打印所有内存块信息（调试用）
pub fn dump_blocks() {
    if !is_initialized() {
        error_print!("Early allocator not initialized");
        return;
    }
    
    unsafe {
        if let Some(ref allocator) = EARLY_ALLOCATOR {
            allocator.dump_blocks();
        }
    }
}

/// 准备接管数据
/// 
/// # 返回值
/// 返回接管信息，如果分配器未初始化则返回None
pub fn prepare_handover() -> Option<HandoverInfo> {
    if !is_initialized() {
        return None;
    }
    
    unsafe {
        if let Some(ref mut allocator) = EARLY_ALLOCATOR {
            Some(allocator.prepare_handover())
        } else {
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
    
    unsafe {
        if let Some(ref mut allocator) = EARLY_ALLOCATOR {
            allocator.freeze();
            info_print!("Early allocator frozen");
            Ok(())
        } else {
            Err(AllocError::NotInitialized)
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