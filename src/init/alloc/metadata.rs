// 早期分配器元数据管理
// 定义块头、统计信息等数据结构

use super::handover::AllocPurpose;

// 块头魔数
pub const BLOCK_MAGIC: u32 = 0xB10C4EA0; // BLOCK HEAD

/// 块状态枚举
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockStatus {
    Free,      // 空闲
    Allocated, // 已分配
}

/// 内存块头结构
/// 每个分配的内存块都有一个头部，包含管理信息
#[repr(C)]
pub struct BlockHeader {
    /// 块大小（不包括头部）
    pub size: usize,
    
    /// 块状态
    pub status: BlockStatus,
    
    /// 下一个空闲块指针（仅在空闲时使用）
    pub next_free: Option<*mut BlockHeader>,
    
    /// 魔数，用于验证块完整性
    pub magic: u32,
    
    /// 分配ID，用于调试
    pub alloc_id: u64,
    
    /// 分配用途
    pub purpose: AllocPurpose,
    
    /// 填充字节，确保头部大小正确
    #[cfg(target_pointer_width = "64")]
    pub padding: [u32; 3], // 64位系统需要12字节填充
    
    #[cfg(target_pointer_width = "32")]
    pub padding: [u32; 5], // 32位系统需要20字节填充
}

/// 分配器统计信息
#[derive(Debug, Clone)]
pub struct AllocStats {
    /// 总内存大小
    pub total_size: usize,
    
    /// 已使用大小
    pub used_size: usize,
    
    /// 空闲大小
    pub free_size: usize,
    
    /// 当前分配块数
    pub alloc_count: usize,
    
    /// 当前空闲块数
    pub free_count: usize,
    
    /// 总分配次数
    pub total_allocs: u64,
    
    /// 总释放次数
    pub total_frees: u64,
}

impl AllocStats {
    /// 创建新的统计信息
    pub fn new(total_size: usize) -> Self {
        Self {
            total_size,
            used_size: 0,
            free_size: total_size,
            alloc_count: 0,
            free_count: 0,
            total_allocs: 0,
            total_frees: 0,
        }
    }
    
    /// 获取内存使用率（百分比）
    pub fn usage_percent(&self) -> u8 {
        if self.total_size == 0 {
            return 0;
        }
        ((self.used_size as f32 / self.total_size as f32) * 100.0) as u8
    }
    
    /// 获取碎片率估算（百分比）
    /// 基于空闲块数量的简单估算
    pub fn fragmentation_estimate(&self) -> u8 {
        if self.free_count == 0 {
            return 0;
        }
        
        // 理想情况下，所有空闲内存应该在一个大块中
        // 空闲块越多，碎片化越严重
        let ideal_free_blocks = 1;
        let actual_free_blocks = self.free_count;
        
        if actual_free_blocks <= ideal_free_blocks {
            return 0;
        }
        
        let fragmentation = ((actual_free_blocks - ideal_free_blocks) as f32 / 
                           actual_free_blocks as f32) * 100.0;
        fragmentation.min(100.0) as u8
    }
    
    /// 打印统计信息摘要
    pub fn print_summary(&self) {
        use crate::println;
        
        println!("=== Memory Statistics ===");
        println!("Total size: {} KB", self.total_size / 1024);
        println!("Used: {} KB ({} bytes)", self.used_size / 1024, self.used_size);
        println!("Free: {} KB ({} bytes)", self.free_size / 1024, self.free_size);
        println!("Usage: {}%", self.usage_percent());
        println!("Allocated blocks: {}", self.alloc_count);
        println!("Free blocks: {}", self.free_count);
        println!("Total allocations: {}", self.total_allocs);
        println!("Total deallocations: {}", self.total_frees);
        println!("Fragmentation (est.): {}%", self.fragmentation_estimate());
        println!("=======================");
    }
}

/// 块头验证
impl BlockHeader {
    /// 验证块头完整性
    pub fn validate(&self) -> bool {
        self.magic == BLOCK_MAGIC && self.size > 0
    }
    
    /// 计算块的总大小（包括头部）
    pub fn total_size(&self) -> usize {
        self.size + core::mem::size_of::<BlockHeader>()
    }
    
    /// 获取用户数据起始地址
    pub fn user_data_addr(&self) -> usize {
        (self as *const BlockHeader as usize) + core::mem::size_of::<BlockHeader>()
    }
    
    /// 设置分配用途
    pub fn set_purpose(&mut self, purpose: AllocPurpose) {
        self.purpose = purpose;
    }
}

// 确保块头大小是16字节的倍数，便于对齐
const _: () = {
    assert!(core::mem::size_of::<BlockHeader>() % 16 == 0,
            "BlockHeader size must be multiple of 16");
};

/// 内存块的迭代器
/// 用于遍历堆中的所有块
pub struct BlockIterator {
    current: usize,
    end: usize,
}

impl BlockIterator {
    /// 创建新的块迭代器
    pub fn new(start: usize, end: usize) -> Self {
        Self { current: start, end }
    }
}

impl Iterator for BlockIterator {
    type Item = *const BlockHeader;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            return None;
        }
        
        let header = self.current as *const BlockHeader;
        
        unsafe {
            // 验证块头
            if (*header).magic == BLOCK_MAGIC {
                self.current += (*header).total_size();
                Some(header)
            } else {
                // 无效块，停止迭代
                None
            }
        }
    }
}

/// 块验证器
/// 用于检查堆的完整性
pub struct BlockValidator {
    heap_start: usize,
    heap_end: usize,
}

impl BlockValidator {
    /// 创建新的验证器
    pub fn new(heap_start: usize, heap_end: usize) -> Self {
        Self { heap_start, heap_end }
    }
    
    /// 验证整个堆的完整性
    pub fn validate_heap(&self) -> Result<(), &'static str> {
        let mut current = self.heap_start;
        let mut block_count = 0;
        
        while current < self.heap_end {
            let header = current as *const BlockHeader;
            
            unsafe {
                // 检查魔数
                if (*header).magic != BLOCK_MAGIC {
                    return Err("Invalid block magic");
                }
                
                // 检查大小
                if (*header).size == 0 {
                    return Err("Block size is zero");
                }
                
                // 检查边界
                let block_end = current + (*header).total_size();
                if block_end > self.heap_end {
                    return Err("Block extends beyond heap boundary");
                }
                
                current = block_end;
                block_count += 1;
            }
            
            // 防止无限循环
            if block_count > 10000 {
                return Err("Too many blocks, possible corruption");
            }
        }
        
        // 确保正好到达堆末尾
        if current != self.heap_end {
            return Err("Heap blocks do not fill entire heap");
        }
        
        Ok(())
    }
    
    /// 验证单个块
    pub fn validate_block(&self, block: *const BlockHeader) -> Result<(), &'static str> {
        let block_addr = block as usize;
        
        // 检查地址范围
        if block_addr < self.heap_start || block_addr >= self.heap_end {
            return Err("Block outside heap range");
        }
        
        unsafe {
            // 检查魔数
            if (*block).magic != BLOCK_MAGIC {
                return Err("Invalid block magic");
            }
            
            // 检查大小
            if (*block).size == 0 {
                return Err("Block size is zero");
            }
            
            // 检查块不会超出堆边界
            let block_end = block_addr + (*block).total_size();
            if block_end > self.heap_end {
                return Err("Block extends beyond heap boundary");
            }
        }
        
        Ok(())
    }
}