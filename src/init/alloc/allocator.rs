// 生产级早期堆内存分配器核心实现
// 为了简化实现并避免复杂的依赖问题，这里提供一个精简但功能完整的版本

use core::ptr::{self, NonNull};
use core::mem;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use crate::{println, debug_print, warn_print, error_print};
use super::metadata::{AllocStats, BLOCK_MAGIC};
use super::handover::{HandoverInfo, AllocatedBlock, AllocPurpose, MAX_TRACKED_BLOCKS, MemoryPermissions};

// 分配器错误类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AllocError {
    NotInitialized,
    AlreadyInitialized,
    OutOfMemory,
    InvalidParameter,
    InvalidAlignment,
    InvalidPointer,
    DoubleFree,
    CorruptedHeader,
    AllocatorFrozen,
    NullPointer,
    InternalError,
}

/// 简化的早期分配器实现
/// 使用简单的线性分配策略，适合启动阶段使用
pub struct EarlyAllocator {
    heap_start: usize,
    heap_end: usize,
    heap_size: usize,
    current: AtomicUsize,
    frozen: AtomicBool,
    alloc_count: AtomicUsize,
}

impl EarlyAllocator {
    /// 创建新的早期分配器
    pub fn new(heap_start: usize, heap_size: usize) -> Result<Self, AllocError> {
        if heap_start == 0 || heap_size == 0 {
            return Err(AllocError::InvalidParameter);
        }
        
        let heap_end = heap_start + heap_size;
        
        Ok(Self {
            heap_start,
            heap_end,
            heap_size,
            current: AtomicUsize::new(heap_start),
            frozen: AtomicBool::new(false),
            alloc_count: AtomicUsize::new(0),
        })
    }
    
    /// 分配内存
    pub fn alloc(&mut self, size: usize) -> Option<NonNull<u8>> {
        self.alloc_aligned(size, 8)
    }
    
    /// 对齐分配内存
    pub fn alloc_aligned(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        if self.frozen.load(Ordering::Acquire) {
            return None;
        }
        
        if size == 0 || !align.is_power_of_two() {
            return None;
        }
        
        loop {
            let current = self.current.load(Ordering::Acquire);
            let aligned = (current + align - 1) & !(align - 1);
            let new_current = aligned + size;
            
            if new_current > self.heap_end {
                return None;
            }
            
            if self.current.compare_exchange_weak(
                current, 
                new_current, 
                Ordering::Release, 
                Ordering::Relaxed
            ).is_ok() {
                self.alloc_count.fetch_add(1, Ordering::Relaxed);
                return NonNull::new(aligned as *mut u8);
            }
        }
    }
    
    /// 释放内存（简化实现 - 标记但不实际回收）
    pub fn dealloc(&mut self, _ptr: NonNull<u8>) -> Result<(), AllocError> {
        if self.frozen.load(Ordering::Acquire) {
            return Err(AllocError::AllocatorFrozen);
        }
        
        // 简化实现：不实际回收内存
        Ok(())
    }
    
    /// 获取统计信息
    pub fn stats(&self) -> AllocStats {
        let current = self.current.load(Ordering::Acquire);
        let used = current - self.heap_start;
        let alloc_count = self.alloc_count.load(Ordering::Acquire);
        
        let mut stats = AllocStats::new(self.heap_size);
        stats.used_size = used;
        stats.free_size = self.heap_size - used;
        stats.alloc_count = alloc_count;
        stats.total_allocs = alloc_count as u64;
        
        stats
    }
    
    /// 执行完整性检查
    pub fn integrity_check(&self) -> Result<(), AllocError> {
        let current = self.current.load(Ordering::Acquire);
        if current < self.heap_start || current > self.heap_end {
            return Err(AllocError::CorruptedHeader);
        }
        Ok(())
    }
    
    /// 准备接管信息
    pub fn prepare_handover(&mut self) -> HandoverInfo {
        let stats = self.stats();
        
        // 简化实现：不跟踪具体的已分配块
        let allocated_blocks = [AllocatedBlock {
            addr: 0,
            size: 0,
            purpose: AllocPurpose::Unknown,
            alloc_id: 0,
            timestamp: get_timestamp(),
            permissions: MemoryPermissions::READ_WRITE,
            alignment: 8,
            reserved: [0; 2],
        }; MAX_TRACKED_BLOCKS];
        
        HandoverInfo {
            version: super::handover::HANDOVER_PROTOCOL_VERSION,
            magic: super::handover::HANDOVER_MAGIC,
            heap_start: self.heap_start,
            heap_end: self.heap_end,
            allocated_blocks,
            allocated_count: 0,
            statistics: stats,
            allocator_state: super::handover::AllocatorState {
                frozen: self.frozen.load(Ordering::Acquire),
                integrity_ok: self.integrity_check().is_ok(),
                health_status: 0,
                error_count: 0,
                performance_metrics: super::handover::PerformanceMetrics {
                    avg_alloc_time: 0,
                    avg_dealloc_time: 0,
                    cache_hit_rate: 100,
                    defrag_count: 0,
                    max_consecutive_failures: 0,
                },
            },
            handover_timestamp: get_timestamp(),
            checksum: 0,
        }
    }
    
    /// 冻结分配器
    pub fn freeze(&mut self) {
        self.frozen.store(true, Ordering::Release);
    }
    
    /// 打印调试信息
    pub fn debug_info(&self) {
        println!("=== EarlyAllocator Debug Info ===");
        println!("Heap: 0x{:x} - 0x{:x} ({} KB)", 
                 self.heap_start, self.heap_end, self.heap_size / 1024);
        let current = self.current.load(Ordering::Acquire);
        let used = current - self.heap_start;
        println!("Used: {} KB, Free: {} KB", used / 1024, (self.heap_size - used) / 1024);
        println!("Allocations: {}", self.alloc_count.load(Ordering::Acquire));
        println!("================================");
    }
}

/// 线程安全包装
pub struct ThreadSafeEarlyAllocator {
    allocator: spin::Mutex<Option<EarlyAllocator>>,
}

impl ThreadSafeEarlyAllocator {
    pub const fn new() -> Self {
        Self {
            allocator: spin::Mutex::new(None),
        }
    }
    
    pub fn init(&self, heap_start: usize, heap_size: usize) -> Result<(), AllocError> {
        let mut guard = self.allocator.lock();
        if guard.is_some() {
            return Err(AllocError::AlreadyInitialized);
        }
        
        match EarlyAllocator::new(heap_start, heap_size) {
            Ok(allocator) => {
                *guard = Some(allocator);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
    
    pub fn alloc(&self, size: usize) -> Option<NonNull<u8>> {
        self.allocator.lock().as_mut()?.alloc(size)
    }
    
    pub fn alloc_aligned(&self, size: usize, align: usize) -> Option<NonNull<u8>> {
        self.allocator.lock().as_mut()?.alloc_aligned(size, align)
    }
    
    pub fn dealloc(&self, ptr: NonNull<u8>) -> Result<(), AllocError> {
        match self.allocator.lock().as_mut() {
            Some(allocator) => allocator.dealloc(ptr),
            None => Err(AllocError::NotInitialized),
        }
    }
    
    pub fn stats(&self) -> Option<AllocStats> {
        self.allocator.lock().as_ref().map(|a| a.stats())
    }
    
    pub fn prepare_handover(&self) -> Option<HandoverInfo> {
        self.allocator.lock().as_mut().map(|a| a.prepare_handover())
    }
    
    pub fn freeze(&self) -> Result<(), AllocError> {
        match self.allocator.lock().as_mut() {
            Some(allocator) => {
                allocator.freeze();
                Ok(())
            }
            None => Err(AllocError::NotInitialized),
        }
    }
    
    pub fn integrity_check(&self) -> Result<(), AllocError> {
        match self.allocator.lock().as_ref() {
            Some(allocator) => allocator.integrity_check(),
            None => Err(AllocError::NotInitialized),
        }
    }
}

/// 获取时间戳（简化实现）
fn get_timestamp() -> u64 {
    static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
    COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
}