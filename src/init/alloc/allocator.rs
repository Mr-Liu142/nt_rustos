// 早期堆内存分配器核心实现
// 使用简化的块链表算法，支持内存回收

use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};
use crate::{println, debug_print};
use super::metadata::{BlockHeader, BlockStatus, AllocStats, BLOCK_MAGIC};
use super::handover::{HandoverInfo, AllocatedBlock, AllocPurpose};

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
}

// 块大小级别（字节）
const BLOCK_SIZES: [usize; 8] = [32, 64, 128, 256, 512, 1024, 2048, 4096];
const NUM_BLOCK_SIZES: usize = BLOCK_SIZES.len();

// 最小和最大块大小
const MIN_BLOCK_SIZE: usize = BLOCK_SIZES[0];
const MAX_BLOCK_SIZE: usize = BLOCK_SIZES[NUM_BLOCK_SIZES - 1];

// 分配器元数据魔数
const ALLOCATOR_MAGIC: u32 = 0xEA110C88; // Early ALLOCator

/// 早期内存分配器结构体
pub struct EarlyAllocator {
    // 基本信息
    heap_start: usize,
    heap_end: usize,
    heap_size: usize,
    
    // 空闲链表头（每个大小级别一个）
    free_lists: [Option<*mut BlockHeader>; NUM_BLOCK_SIZES],
    
    // 统计信息
    stats: AllocStats,
    
    // 状态标志
    frozen: AtomicBool,
    magic: u32,
}

impl EarlyAllocator {
    /// 创建新的早期分配器
    /// 
    /// # 参数
    /// * `heap_start` - 堆起始地址
    /// * `heap_size` - 堆大小
    /// 
    /// # 返回值
    /// 成功返回分配器实例，失败返回错误
    pub fn new(heap_start: usize, heap_size: usize) -> Result<Self, AllocError> {
        // 检查参数
        if heap_start == 0 || heap_size == 0 {
            return Err(AllocError::InvalidParameter);
        }
        
        // 确保堆大小至少能容纳一个最大块
        if heap_size < MAX_BLOCK_SIZE + core::mem::size_of::<BlockHeader>() {
            return Err(AllocError::InvalidParameter);
        }
        
        let heap_end = heap_start + heap_size;
        
        // 创建分配器实例
        let mut allocator = Self {
            heap_start,
            heap_end,
            heap_size,
            free_lists: [None; NUM_BLOCK_SIZES],
            stats: AllocStats {
                total_size: heap_size,
                used_size: 0,
                free_size: heap_size,
                alloc_count: 0,
                free_count: 0,
                total_allocs: 0,
                total_frees: 0,
            },
            frozen: AtomicBool::new(false),
            magic: ALLOCATOR_MAGIC,
        };
        
        // 初始化堆
        allocator.init_heap();
        
        Ok(allocator)
    }
    
    /// 初始化堆内存
    fn init_heap(&mut self) {
        // 将整个堆作为一个大的空闲块
        let mut current = self.heap_start;
        let chunk_size = MAX_BLOCK_SIZE + core::mem::size_of::<BlockHeader>();
        
        while current + chunk_size <= self.heap_end {
            // 创建一个最大尺寸的空闲块
            let header = current as *mut BlockHeader;
            unsafe {
                (*header).size = MAX_BLOCK_SIZE;
                (*header).status = BlockStatus::Free;
                (*header).next_free = None;
                (*header).magic = BLOCK_MAGIC;
                (*header).alloc_id = 0;
                (*header).purpose = AllocPurpose::Unknown;
                (*header).padding = 0;
            }
            
            // 添加到对应的空闲链表
            self.add_to_free_list(header, NUM_BLOCK_SIZES - 1);
            
            current += chunk_size;
        }
        
        // 更新统计信息
        let actual_usable = ((self.heap_end - self.heap_start) / chunk_size) * MAX_BLOCK_SIZE;
        self.stats.free_size = actual_usable;
        self.stats.total_size = actual_usable;
    }
    
    /// 分配内存
    /// 
    /// # 参数
    /// * `size` - 要分配的字节数
    /// 
    /// # 返回值
    /// 成功返回内存地址，失败返回None
    pub fn alloc(&mut self, size: usize) -> Option<*mut u8> {
        // 检查是否已冻结
        if self.frozen.load(Ordering::Acquire) {
            return None;
        }
        
        // 检查大小是否有效
        if size == 0 || size > MAX_BLOCK_SIZE {
            return None;
        }
        
        // 找到合适的块大小级别
        let size_index = self.find_size_index(size);
        let block_size = BLOCK_SIZES[size_index];
        
        // 从对应级别或更大级别寻找空闲块
        for i in size_index..NUM_BLOCK_SIZES {
            if let Some(block) = self.alloc_from_list(i) {
                // 如果块太大，需要分割
                if i > size_index {
                    self.split_block(block, size_index, i);
                }
                
                // 标记为已分配
                unsafe {
                    (*block).status = BlockStatus::Allocated;
                    (*block).alloc_id = self.stats.total_allocs;
                }
                
                // 更新统计
                self.stats.used_size += block_size;
                self.stats.free_size -= block_size;
                self.stats.alloc_count += 1;
                self.stats.total_allocs += 1;
                
                // 返回用户数据区域的指针
                let user_ptr = unsafe { (block as *mut u8).add(core::mem::size_of::<BlockHeader>()) };
                return Some(user_ptr);
            }
        }
        
        // 没有找到合适的空闲块
        None
    }
    
    /// 对齐分配内存
    pub fn alloc_aligned(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        // 简单实现：分配更大的块以满足对齐要求
        let header_size = core::mem::size_of::<BlockHeader>();
        let total_size = size + align + header_size;
        
        if let Some(ptr) = self.alloc(total_size) {
            // 计算对齐后的地址
            let addr = ptr as usize;
            let aligned_addr = (addr + align - 1) & !(align - 1);
            Some(aligned_addr as *mut u8)
        } else {
            None
        }
    }
    
    /// 释放内存
    /// 
    /// # 参数
    /// * `ptr` - 要释放的内存地址
    /// 
    /// # 返回值
    /// 成功返回Ok(())，失败返回错误
    pub fn dealloc(&mut self, ptr: *mut u8) -> Result<(), AllocError> {
        // 检查指针有效性
        if ptr.is_null() {
            return Err(AllocError::NullPointer);
        }
        
        let addr = ptr as usize;
        if addr < self.heap_start || addr >= self.heap_end {
            return Err(AllocError::InvalidPointer);
        }
        
        // 获取块头
        let header_addr = addr - core::mem::size_of::<BlockHeader>();
        let header = header_addr as *mut BlockHeader;
        
        // 验证块头
        unsafe {
            if (*header).magic != BLOCK_MAGIC {
                return Err(AllocError::CorruptedHeader);
            }
            
            if (*header).status == BlockStatus::Free {
                return Err(AllocError::DoubleFree);
            }
            
            // 标记为空闲
            (*header).status = BlockStatus::Free;
            (*header).next_free = None;
            
            // 找到对应的大小级别
            let size = (*header).size;
            let size_index = self.find_size_index(size);
            
            // 添加到空闲链表
            self.add_to_free_list(header, size_index);
            
            // 更新统计
            self.stats.used_size -= size;
            self.stats.free_size += size;
            self.stats.free_count += 1;
            self.stats.total_frees += 1;
            
            // 尝试合并相邻的空闲块
            self.try_merge_blocks(header, size_index);
        }
        
        Ok(())
    }
    
    /// 获取统计信息
    pub fn stats(&self) -> AllocStats {
        self.stats.clone()
    }
    
    /// 打印所有内存块信息
    pub fn dump_blocks(&self) {
        println!("=== Early Allocator Block Dump ===");
        println!("Heap range: 0x{:x} - 0x{:x}", self.heap_start, self.heap_end);
        println!("Total size: {} bytes", self.stats.total_size);
        println!("Used: {} bytes, Free: {} bytes", self.stats.used_size, self.stats.free_size);
        
        // 遍历所有块
        let mut current = self.heap_start;
        let mut block_count = 0;
        
        while current < self.heap_end {
            let header = current as *const BlockHeader;
            unsafe {
                if (*header).magic == BLOCK_MAGIC {
                    println!("Block {}: addr=0x{:x}, size={}, status={:?}, alloc_id={}", 
                             block_count, current, (*header).size, (*header).status, (*header).alloc_id);
                    current += (*header).size + core::mem::size_of::<BlockHeader>();
                    block_count += 1;
                } else {
                    // 跳过无效块
                    current += core::mem::size_of::<BlockHeader>();
                }
            }
            
            // 防止无限循环
            if block_count > 1000 {
                println!("Too many blocks, stopping dump");
                break;
            }
        }
        
        println!("Total blocks found: {}", block_count);
        println!("=================================");
    }
    
    /// 准备接管信息
    pub fn prepare_handover(&mut self) -> HandoverInfo {
        let mut allocated_blocks = Vec::new();
        
        // 遍历所有块，收集已分配的块
        let mut current = self.heap_start;
        
        while current < self.heap_end {
            let header = current as *const BlockHeader;
            unsafe {
                if (*header).magic == BLOCK_MAGIC && (*header).status == BlockStatus::Allocated {
                    allocated_blocks.push(AllocatedBlock {
                        addr: current + core::mem::size_of::<BlockHeader>(),
                        size: (*header).size,
                        purpose: (*header).purpose,
                        alloc_id: (*header).alloc_id,
                    });
                }
                
                if (*header).magic == BLOCK_MAGIC {
                    current += (*header).size + core::mem::size_of::<BlockHeader>();
                } else {
                    current += core::mem::size_of::<BlockHeader>();
                }
            }
        }
        
        HandoverInfo {
            heap_start: self.heap_start,
            heap_end: self.heap_end,
            allocated_blocks,
            statistics: self.stats.clone(),
        }
    }
    
    /// 冻结分配器
    pub fn freeze(&mut self) {
        self.frozen.store(true, Ordering::Release);
    }
    
    // 内部辅助方法
    
    /// 找到适合给定大小的块级别索引
    fn find_size_index(&self, size: usize) -> usize {
        for (i, &block_size) in BLOCK_SIZES.iter().enumerate() {
            if size <= block_size {
                return i;
            }
        }
        NUM_BLOCK_SIZES - 1
    }
    
    /// 从指定级别的空闲链表分配块
    fn alloc_from_list(&mut self, size_index: usize) -> Option<*mut BlockHeader> {
        if let Some(block) = self.free_lists[size_index] {
            unsafe {
                // 从链表中移除
                self.free_lists[size_index] = (*block).next_free;
                (*block).next_free = None;
            }
            Some(block)
        } else {
            None
        }
    }
    
    /// 添加块到空闲链表
    fn add_to_free_list(&mut self, block: *mut BlockHeader, size_index: usize) {
        unsafe {
            (*block).next_free = self.free_lists[size_index];
            self.free_lists[size_index] = Some(block);
        }
    }
    
    /// 分割块
    fn split_block(&mut self, block: *mut BlockHeader, target_index: usize, current_index: usize) {
        // 从大块中分割出小块
        let mut remaining_index = current_index;
        let mut current_block = block;
        
        while remaining_index > target_index {
            unsafe {
                // 计算新块的位置
                let new_size = BLOCK_SIZES[remaining_index - 1];
                let new_block_addr = (current_block as usize) + 
                                   core::mem::size_of::<BlockHeader>() + new_size;
                let new_block = new_block_addr as *mut BlockHeader;
                
                // 创建新块头
                (*new_block).size = new_size;
                (*new_block).status = BlockStatus::Free;
                (*new_block).next_free = None;
                (*new_block).magic = BLOCK_MAGIC;
                (*new_block).alloc_id = 0;
                (*new_block).purpose = AllocPurpose::Unknown;
                (*new_block).padding = 0;
                
                // 更新当前块的大小
                (*current_block).size = new_size;
                
                // 将新块添加到空闲链表
                self.add_to_free_list(new_block, remaining_index - 1);
            }
            
            remaining_index -= 1;
        }
    }
    
    /// 尝试合并相邻的空闲块
    fn try_merge_blocks(&mut self, _block: *mut BlockHeader, _size_index: usize) {
        // 简化实现：暂不实现块合并
        // 在实际使用中，早期分配器的生命周期较短，碎片化问题不严重
    }
}