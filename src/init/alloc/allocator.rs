// 生产级早期堆内存分配器核心实现
// 使用基于地址排序的双向空闲链表的分配策略

use core::ptr::{self, NonNull};
use core::mem;
use super::metadata::{BlockHeader, AllocStats, BlockStatus, BLOCK_MAGIC};
use super::handover::{HandoverInfo, AllocatedBlock, AllocPurpose, MAX_TRACKED_BLOCKS, MemoryPermissions};
use super::global::advanced;
use crate::{error_print, warn_print, debug_print};

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

/// 空闲内存块结构
/// 用于构成双向链表，存储在空闲块的头部之后
#[repr(C)]
struct FreeBlock {
    next: *mut FreeBlock,
    prev: *mut FreeBlock,
}

/// 生产级早期分配器实现
pub struct EarlyAllocator {
    heap_start: usize,
    heap_end: usize,
    free_list_head: *mut FreeBlock,
    stats: AllocStats,
    frozen: bool,
    next_alloc_id: u64,
}

// 通过手动实现 Send Trait，我们向编译器保证：
// 尽管 EarlyAllocator 内部含有裸指针，但由于所有对它的访问
// 都将通过 Mutex 进行同步，因此在线程间传递所有权是安全的。
unsafe impl Send for EarlyAllocator {}

impl EarlyAllocator {
    /// 创建新的早期分配器
    pub fn new(heap_start: usize, heap_size: usize) -> Result<Self, AllocError> {
        if heap_start == 0 || heap_size < Self::min_heap_size() {
            return Err(AllocError::InvalidParameter);
        }
        
        let heap_end = heap_start + heap_size;
        
        // 初始化第一个块头
        let initial_header = heap_start as *mut BlockHeader;
        unsafe {
            *initial_header = BlockHeader::new(heap_size - mem::size_of::<BlockHeader>(), BlockStatus::Free);
        }

        // 初始化第一个空闲块
        let initial_free_block = (heap_start + mem::size_of::<BlockHeader>()) as *mut FreeBlock;
        unsafe {
            *initial_free_block = FreeBlock {
                next: ptr::null_mut(),
                prev: ptr::null_mut(),
            };
        }

        let mut stats = AllocStats::new(heap_size);
        stats.free_size = heap_size;
        stats.free_count = 1;
        stats.max_free_block_size = heap_size;

        Ok(Self {
            heap_start,
            heap_end,
            free_list_head: initial_free_block,
            stats,
            frozen: false,
            next_alloc_id: 1,
        })
    }
    
    /// 分配内存
    pub fn alloc(&mut self, size: usize) -> Option<NonNull<u8>> {
        self.alloc_aligned(size, mem::align_of::<usize>())
    }
    
    /// 对齐分配内存
    pub fn alloc_aligned(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        if self.frozen {
            self.stats.record_alloc_failure();
            return None;
        }

        if size == 0 || !align.is_power_of_two() {
            self.stats.record_alloc_failure();
            return None;
        }

        // 规范化请求的大小，至少要能容纳一个FreeBlock
        let alloc_size = size.max(mem::size_of::<FreeBlock>());

        // 寻找合适的空闲块
        if let Some((block_header, user_addr)) = self.find_free_block(alloc_size, align) {
            let block_addr = block_header as usize;
            let block_size = unsafe { (*block_header).size };
            let free_block = unsafe { &mut *((block_addr + mem::size_of::<BlockHeader>()) as *mut FreeBlock) };
            
            // 从空闲链表中移除
            self.remove_from_free_list(free_block);
            self.stats.free_size -= block_size + mem::size_of::<BlockHeader>();
            self.stats.free_count -= 1;

            let required_size = user_addr - block_addr + alloc_size;

            // 如果剩余空间足够大，则分裂块
            if block_size >= required_size + Self::min_block_size() {
                // 原块分裂为两部分：已分配块 和 新的空闲块
                let new_free_block_addr = block_addr + required_size;
                let new_free_block_size = block_size - required_size;

                unsafe {
                    // 更新原块头为已分配
                    (*block_header).size = required_size - mem::size_of::<BlockHeader>();
                    (*block_header).status = BlockStatus::Allocated;
                    (*block_header).alloc_id = self.next_alloc_id;
                    self.next_alloc_id += 1;
                    (*block_header).update_timestamp();
                    (*block_header).update_checksum();

                    // 创建新的空闲块头
                    let new_header = new_free_block_addr as *mut BlockHeader;
                    *new_header = BlockHeader::new(new_free_block_size, BlockStatus::Free);

                    // 创建新的FreeBlock并插入链表
                    let new_free = (new_free_block_addr + mem::size_of::<BlockHeader>()) as *mut FreeBlock;
                    self.insert_into_free_list(new_free);
                }
                self.stats.record_split(new_free_block_size);
                self.stats.free_size += new_free_block_size + mem::size_of::<BlockHeader>();
                self.stats.free_count += 1;
            } else {
                // 不分裂，整个块都分配
                unsafe {
                    (*block_header).status = BlockStatus::Allocated;
                    (*block_header).alloc_id = self.next_alloc_id;
                    self.next_alloc_id += 1;
                    (*block_header).update_timestamp();
                    (*block_header).update_checksum();
                }
            }

            self.stats.record_alloc(unsafe { (*block_header).size });
            return NonNull::new(user_addr as *mut u8);
        }

        self.stats.record_alloc_failure();
        None
    }
    
    /// 释放内存
    pub fn dealloc(&mut self, ptr: NonNull<u8>) -> Result<(), AllocError> {
        if self.frozen { return Err(AllocError::AllocatorFrozen); }

        let user_ptr = ptr.as_ptr() as usize;

        if user_ptr < self.heap_start || user_ptr > self.heap_end {
            return Err(AllocError::InvalidPointer);
        }
        
        let header_ptr = (user_ptr - mem::size_of::<BlockHeader>()) as *mut BlockHeader;
        
        if !unsafe { (*header_ptr).validate() } {
            self.stats.record_corruption();
            return Err(AllocError::CorruptedHeader);
        }
        
        if unsafe { (*header_ptr).status == BlockStatus::Free } {
            self.stats.record_double_free();
            return Err(AllocError::DoubleFree);
        }

        let block_size = unsafe { (*header_ptr).size };
        self.stats.record_dealloc(block_size);
        self.stats.free_size += block_size + mem::size_of::<BlockHeader>();
        self.stats.free_count += 1;
        
        unsafe {
            (*header_ptr).status = BlockStatus::Free;
            (*header_ptr).update_timestamp();
            (*header_ptr).update_checksum();
            
            let free_block = (header_ptr as usize + mem::size_of::<BlockHeader>()) as *mut FreeBlock;
            self.insert_into_free_list(free_block);
            self.coalesce(free_block);
        }
        
        Ok(())
    }
    
    /// 获取统计信息
    pub fn stats(&self) -> AllocStats {
        self.stats.clone()
    }
    
    /// 执行完整性检查
    pub fn integrity_check(&self) -> Result<(), AllocError> {
        let mut current_addr = self.heap_start;
        while current_addr < self.heap_end {
            let header = current_addr as *const BlockHeader;
            unsafe {
                if !(*header).validate() {
                    error_print!("Integrity check failed at 0x{:x}", current_addr);
                    return Err(AllocError::CorruptedHeader);
                }
                current_addr += (*header).total_size();
            }
        }
        if current_addr != self.heap_end {
            error_print!("Heap corruption: size mismatch. Expected end 0x{:x}, got 0x{:x}", self.heap_end, current_addr);
            return Err(AllocError::InternalError);
        }
        Ok(())
    }
    
    /// 准备接管信息
    pub fn prepare_handover(&mut self) -> Option<advanced::EarlyBox<HandoverInfo>> {
        let stats = self.stats();
        let mut info = HandoverInfo::new(self.heap_start, self.heap_end - self.heap_start, stats);

        let mut current_addr = self.heap_start;
        while current_addr < self.heap_end {
            let header = current_addr as *const BlockHeader;
            unsafe {
                if (*header).status == BlockStatus::Allocated {
                    if info.allocated_count < MAX_TRACKED_BLOCKS {
                        let block = AllocatedBlock {
                            addr: (*header).user_data_addr(),
                            size: (*header).size,
                            purpose: (*header).purpose,
                            alloc_id: (*header).alloc_id,
                            timestamp: (*header).timestamp,
                            permissions: MemoryPermissions::READ_WRITE,
                            alignment: 8,
                            reserved: [0; 2],
                        };
                        info.allocated_blocks[info.allocated_count] = block;
                        info.allocated_count += 1;
                    } else {
                        warn_print!("MAX_TRACKED_BLOCKS limit reached, handover info is incomplete.");
                        break;
                    }
                }
                current_addr += (*header).total_size();
            }
        }
        info.update_checksum();
        advanced::EarlyBox::new(info)
    }
    
    /// 冻结分配器
    pub fn freeze(&mut self) {
        self.frozen = true;
    }
    
    /// 设置分配用途
    pub fn set_purpose(&mut self, ptr: NonNull<u8>, purpose: AllocPurpose) -> Result<(), AllocError> {
        let user_ptr = ptr.as_ptr() as usize;
        let header_ptr = (user_ptr - mem::size_of::<BlockHeader>()) as *mut BlockHeader;
        unsafe {
            if !(*header_ptr).validate() { return Err(AllocError::CorruptedHeader); }
            if (*header_ptr).status != BlockStatus::Allocated { return Err(AllocError::InvalidPointer); }
            (*header_ptr).purpose = purpose;
            (*header_ptr).update_checksum();
        }
        Ok(())
    }

    fn min_block_size() -> usize {
        mem::size_of::<BlockHeader>() + mem::size_of::<FreeBlock>()
    }

    fn min_heap_size() -> usize {
        Self::min_block_size() * 2
    }

    /// 寻找合适的空闲块 (First-Fit)
    fn find_free_block(&self, size: usize, align: usize) -> Option<(*mut BlockHeader, usize)> {
        let mut current = self.free_list_head;
        while !current.is_null() {
            let header = unsafe { Self::get_header_from_free_block(current) };
            let block_size = unsafe { (*header).size };
            let block_addr = header as usize;

            let user_addr = Self::calculate_aligned_addr(block_addr, align);
            let required_space = user_addr - block_addr + size;
            
            if block_size >= required_space {
                return Some((header, user_addr));
            }
            current = unsafe { (*current).next };
        }
        None
    }

    fn calculate_aligned_addr(block_addr: usize, align: usize) -> usize {
        let data_addr = block_addr + mem::size_of::<BlockHeader>();
        (data_addr + align - 1) & !(align - 1)
    }

    /// 将块从空闲链表中移除
    fn remove_from_free_list(&mut self, block: *mut FreeBlock) {
        unsafe {
            if !(*block).prev.is_null() {
                (*(*block).prev).next = (*block).next;
            } else {
                self.free_list_head = (*block).next;
            }
            if !(*block).next.is_null() {
                (*(*block).next).prev = (*block).prev;
            }
        }
    }

    /// 将块插入到空闲链表中（保持地址有序）
    fn insert_into_free_list(&mut self, block: *mut FreeBlock) {
        let block_addr = unsafe{ Self::get_header_from_free_block(block) } as usize;
        let mut current = self.free_list_head;

        if current.is_null() || (unsafe { Self::get_header_from_free_block(current) } as usize) > block_addr {
            unsafe {
                (*block).next = current;
                (*block).prev = ptr::null_mut();
                if !current.is_null() {
                    (*current).prev = block;
                }
                self.free_list_head = block;
            }
            return;
        }

        while unsafe { !(*current).next.is_null() && (Self::get_header_from_free_block((*current).next) as usize) < block_addr } {
            current = unsafe { (*current).next };
        }

        unsafe {
            (*block).next = (*current).next;
            (*block).prev = current;
            if !(*current).next.is_null() {
                (*(*current).next).prev = block;
            }
            (*current).next = block;
        }
    }

    /// 合并相邻的空闲块
    fn coalesce(&mut self, block: *mut FreeBlock) {
        let header = unsafe { Self::get_header_from_free_block(block) };
        
        // 尝试与下一个块合并
        let next_header_addr = (header as usize) + unsafe { (*header).total_size() };
        if next_header_addr < self.heap_end {
            let next_header = next_header_addr as *mut BlockHeader;
            if unsafe { (*next_header).status == BlockStatus::Free } {
                let next_free = (next_header_addr + mem::size_of::<BlockHeader>()) as *mut FreeBlock;
                self.remove_from_free_list(next_free);
                unsafe {
                    (*header).size += (*next_header).total_size();
                    (*header).update_checksum();
                }
                self.stats.record_merge();
                self.stats.free_count -= 1;
            }
        }
        
        // 尝试与上一个块合并
        if unsafe { !(*block).prev.is_null() } {
            let prev_block = unsafe { (*block).prev };
            let prev_header = unsafe { Self::get_header_from_free_block(prev_block) };
            if (prev_header as usize) + unsafe { (*prev_header).total_size() } == header as usize {
                self.remove_from_free_list(block);
                unsafe {
                    (*prev_header).size += (*header).total_size();
                    (*prev_header).update_checksum();
                }
                self.stats.record_merge();
                self.stats.free_count -= 1;
            }
        }
    }

    unsafe fn get_header_from_free_block(free_block: *mut FreeBlock) -> *mut BlockHeader {
        (free_block as usize - mem::size_of::<BlockHeader>()) as *mut BlockHeader
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
    
    pub fn prepare_handover(&self) -> Option<advanced::EarlyBox<HandoverInfo>> {
        self.allocator.lock().as_mut().and_then(|a| a.prepare_handover())
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

    pub fn set_purpose(&self, ptr: NonNull<u8>, purpose: AllocPurpose) -> Result<(), AllocError> {
        match self.allocator.lock().as_mut() {
            Some(allocator) => allocator.set_purpose(ptr, purpose),
            None => Err(AllocError::NotInitialized),
        }
    }
}

/// 获取时间戳（简化实现）
fn get_timestamp() -> u64 {
    static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
    COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
}