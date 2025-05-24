// 生产级早期分配器元数据管理
// 定义完善的块头、统计信息等数据结构

use super::handover::AllocPurpose;
use core::mem;

// 块头魔数
pub const BLOCK_MAGIC: u32 = 0xB10C4EA0; // BLOCK HEAD

/// 块状态枚举
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockStatus {
    Free,      // 空闲
    Allocated, // 已分配
}

/// 内存块头结构 - 生产级版本
/// 每个分配的内存块都有一个头部，包含完整的管理信息
#[repr(C)]
pub struct BlockHeader {
    /// 块大小（不包括头部）
    pub size: usize,
    
    /// 块状态
    pub status: BlockStatus,
    
    /// 魔数，用于验证块完整性
    pub magic: u32,
    
    /// 分配ID，用于调试和追踪
    pub alloc_id: u64,
    
    /// 分配用途
    pub purpose: AllocPurpose,
    
    /// 分配时间戳（相对时间，用于LRU等算法）
    pub timestamp: u64,
    
    /// 校验和（简单的完整性检查）
    pub checksum: u32,
    
    /// 填充字节，确保头部大小为16字节的倍数
    #[cfg(target_pointer_width = "64")]
    pub padding: [u8; 4],
    
    #[cfg(target_pointer_width = "32")]
    pub padding: [u8; 8],
}

impl BlockHeader {
    /// 创建新的块头
    pub fn new(size: usize, status: BlockStatus) -> Self {
        let mut header = Self {
            size,
            status,
            magic: BLOCK_MAGIC,
            alloc_id: 0,
            purpose: AllocPurpose::Unknown,
            timestamp: get_timestamp(),
            checksum: 0,
            #[cfg(target_pointer_width = "64")]
            padding: [0; 4],
            #[cfg(target_pointer_width = "32")]
            padding: [0; 8],
        };
        
        header.update_checksum();
        header
    }
    
    /// 验证块头完整性
    pub fn validate(&self) -> bool {
        if self.magic != BLOCK_MAGIC {
            return false;
        }
        
        if self.size == 0 {
            return false;
        }
        
        // 验证校验和
        let calculated_checksum = self.calculate_checksum();
        if self.checksum != calculated_checksum {
            return false;
        }
        
        true
    }
    
    /// 计算校验和
    fn calculate_checksum(&self) -> u32 {
        let mut checksum = 0u32;
        
        checksum = checksum.wrapping_add(self.size as u32);
        checksum = checksum.wrapping_add(match self.status {
            BlockStatus::Free => 0x12345678,
            BlockStatus::Allocated => 0x87654321,
        });
        checksum = checksum.wrapping_add(self.magic);
        checksum = checksum.wrapping_add(self.alloc_id as u32);
        checksum = checksum.wrapping_add(self.alloc_id.wrapping_shr(32) as u32);
        checksum = checksum.wrapping_add(self.timestamp as u32);
        checksum = checksum.wrapping_add(self.timestamp.wrapping_shr(32) as u32);
        
        checksum
    }
    
    /// 更新校验和
    pub fn update_checksum(&mut self) {
        self.checksum = self.calculate_checksum();
    }
    
    /// 计算块的总大小（包括头部）
    pub fn total_size(&self) -> usize {
        self.size + mem::size_of::<BlockHeader>()
    }
    
    /// 获取用户数据起始地址
    pub fn user_data_addr(&self) -> usize {
        (self as *const BlockHeader as usize) + mem::size_of::<BlockHeader>()
    }
    
    /// 设置分配用途
    pub fn set_purpose(&mut self, purpose: AllocPurpose) {
        self.purpose = purpose;
        self.update_checksum();
    }
    
    /// 设置分配ID
    pub fn set_alloc_id(&mut self, alloc_id: u64) {
        self.alloc_id = alloc_id;
        self.update_checksum();
    }
    
    /// 更新时间戳
    pub fn update_timestamp(&mut self) {
        self.timestamp = get_timestamp();
        self.update_checksum();
    }
    
    /// 检查块是否过期（用于调试泄漏检测）
    pub fn is_old(&self, threshold: u64) -> bool {
        let current_time = get_timestamp();
        current_time.saturating_sub(self.timestamp) > threshold
    }
}

/// 增强的分配器统计信息
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
    
    /// 失败的分配次数
    pub failed_allocs: u64,
    
    /// 双重释放检测次数
    pub double_free_attempts: u64,
    
    /// 损坏块检测次数
    pub corrupted_blocks: u64,
    
    /// 最大单次分配大小
    pub max_alloc_size: usize,
    
    /// 最小单次分配大小
    pub min_alloc_size: usize,
    
    /// 平均分配大小
    pub avg_alloc_size: usize,
    
    /// 块合并次数
    pub merge_count: usize,
    
    /// 块分割次数
    pub split_count: usize,
    
    /// 合并操作次数
    pub coalesce_count: usize,
    
    /// 峰值内存使用量
    pub peak_used_size: usize,
    
    /// 最大空闲块大小
    pub max_free_block_size: usize,
    
    /// 碎片化程度（百分比）
    pub fragmentation_percent: u8,
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
            failed_allocs: 0,
            double_free_attempts: 0,
            corrupted_blocks: 0,
            max_alloc_size: 0,
            min_alloc_size: usize::MAX,
            avg_alloc_size: 0,
            merge_count: 0,
            split_count: 0,
            coalesce_count: 0,
            peak_used_size: 0,
            max_free_block_size: total_size,
            fragmentation_percent: 0,
        }
    }
    
    /// 记录分配
    pub fn record_alloc(&mut self, size: usize) {
        self.used_size += size;
        self.free_size -= size;
        self.alloc_count += 1;
        self.total_allocs += 1;
        
        // 更新统计
        self.max_alloc_size = self.max_alloc_size.max(size);
        if self.min_alloc_size == usize::MAX {
            self.min_alloc_size = size;
        } else {
            self.min_alloc_size = self.min_alloc_size.min(size);
        }
        
        // 更新平均大小
        if self.total_allocs > 0 {
            self.avg_alloc_size = self.used_size / self.alloc_count;
        }
        
        // 更新峰值
        self.peak_used_size = self.peak_used_size.max(self.used_size);
    }
    
    /// 记录释放
    pub fn record_dealloc(&mut self, size: usize) {
        self.used_size -= size;
        self.free_size += size;
        self.alloc_count = self.alloc_count.saturating_sub(1);
        self.total_frees += 1;
        
        // 更新平均大小
        if self.alloc_count > 0 {
            self.avg_alloc_size = self.used_size / self.alloc_count;
        } else {
            self.avg_alloc_size = 0;
        }
    }
    
    /// 记录分配失败
    pub fn record_alloc_failure(&mut self) {
        self.failed_allocs += 1;
    }
    
    /// 记录双重释放尝试
    pub fn record_double_free(&mut self) {
        self.double_free_attempts += 1;
    }
    
    /// 记录损坏块
    pub fn record_corruption(&mut self) {
        self.corrupted_blocks += 1;
    }
    
    /// 获取内存使用率（百分比）
    pub fn usage_percent(&self) -> u8 {
        if self.total_size == 0 {
            return 0;
        }
        ((self.used_size as f32 / self.total_size as f32) * 100.0) as u8
    }
    
    /// 获取碎片率（百分比）
    pub fn fragmentation_estimate(&self) -> u8 {
        if self.free_count == 0 {
            return 0;
        }
        
        // 基于空闲块数量的碎片估算
        let ideal_free_blocks = if self.free_size > 0 { 1 } else { 0 };
        let actual_free_blocks = self.free_count;
        
        if actual_free_blocks <= ideal_free_blocks {
            return 0;
        }
        
        let fragmentation = ((actual_free_blocks - ideal_free_blocks) as f32 / 
                           actual_free_blocks as f32) * 100.0;
        fragmentation.min(100.0) as u8
    }
    
    /// 获取分配成功率（百分比）
    pub fn success_rate(&self) -> u8 {
        let total_attempts = self.total_allocs + self.failed_allocs;
        if total_attempts == 0 {
            return 100;
        }
        
        ((self.total_allocs as f32 / total_attempts as f32) * 100.0) as u8
    }
    
    /// 获取内存回收率（百分比）
    pub fn reclaim_rate(&self) -> u8 {
        if self.total_allocs == 0 {
            return 100;
        }
        
        ((self.total_frees as f32 / self.total_allocs as f32) * 100.0) as u8
    }
    
    /// 获取平均块生命周期
    pub fn avg_block_lifetime(&self) -> f32 {
        if self.total_frees == 0 {
            return 0.0;
        }
        
        // 简化的生命周期估算
        self.total_allocs as f32 / self.total_frees as f32
    }
    
    /// 打印详细统计信息
    pub fn print_detailed(&self) {
        use crate::println;
        
        println!("=== Detailed Memory Statistics ===");
        
        // 基本信息
        println!("Memory Layout:");
        println!("  Total size: {} KB ({} bytes)", self.total_size / 1024, self.total_size);
        println!("  Used: {} KB ({} bytes, {}%)", 
                 self.used_size / 1024, self.used_size, self.usage_percent());
        println!("  Free: {} KB ({} bytes)", self.free_size / 1024, self.free_size);
        println!("  Peak usage: {} KB ({}%)", 
                 self.peak_used_size / 1024, 
                 (self.peak_used_size * 100 / self.total_size.max(1)) as u8);
        
        // 块统计
        println!("Block Statistics:");
        println!("  Allocated blocks: {}", self.alloc_count);
        println!("  Free blocks: {}", self.free_count);
        println!("  Max free block: {} KB", self.max_free_block_size / 1024);
        
        // 分配统计
        println!("Allocation Statistics:");
        println!("  Total allocations: {}", self.total_allocs);
        println!("  Total deallocations: {}", self.total_frees);
        println!("  Failed allocations: {}", self.failed_allocs);
        println!("  Success rate: {}%", self.success_rate());
        println!("  Reclaim rate: {}%", self.reclaim_rate());
        
        // 大小统计
        println!("Size Statistics:");
        if self.min_alloc_size != usize::MAX {
            println!("  Min allocation: {} bytes", self.min_alloc_size);
        }
        println!("  Max allocation: {} bytes", self.max_alloc_size);
        println!("  Average allocation: {} bytes", self.avg_alloc_size);
        
        // 性能统计
        println!("Performance Statistics:");
        println!("  Block merges: {}", self.merge_count);
        println!("  Block splits: {}", self.split_count);
        println!("  Coalesce operations: {}", self.coalesce_count);
        println!("  Fragmentation: {}%", self.fragmentation_estimate());
        
        // 错误统计
        println!("Error Statistics:");
        println!("  Double free attempts: {}", self.double_free_attempts);
        println!("  Corrupted blocks: {}", self.corrupted_blocks);
        println!("  Average block lifetime: {:.2}", self.avg_block_lifetime());
        
        println!("=====================================");
    }
    
    /// 打印紧凑的统计摘要
    pub fn print_summary(&self) {
        use crate::println;
        
        println!("Memory: {}/{} KB ({}%), Allocs: {}/{} ({}%), Frag: {}%",
                 self.used_size / 1024,
                 self.total_size / 1024,
                 self.usage_percent(),
                 self.total_frees,
                 self.total_allocs,
                 self.success_rate(),
                 self.fragmentation_estimate());
    }
    
    /// 检查是否有异常情况
    pub fn check_health(&self) -> HealthStatus {
        let mut issues = HealthIssues::empty();
        
        // 检查内存使用率
        if self.usage_percent() > 90 {
            issues |= HealthIssues::HIGH_MEMORY_USAGE;
        }
        
        // 检查碎片化
        if self.fragmentation_estimate() > 50 {
            issues |= HealthIssues::HIGH_FRAGMENTATION;
        }
        
        // 检查分配成功率
        if self.success_rate() < 95 {
            issues |= HealthIssues::LOW_SUCCESS_RATE;
        }
        
        // 检查错误率
        if self.double_free_attempts > 0 || self.corrupted_blocks > 0 {
            issues |= HealthIssues::CORRUPTION_DETECTED;
        }
        
        // 检查内存泄漏迹象
        if self.total_allocs > self.total_frees && 
           (self.total_allocs - self.total_frees) > 100 {
            issues |= HealthIssues::POTENTIAL_LEAK;
        }
        
        HealthStatus { issues }
    }
}

/// 健康状态
pub struct HealthStatus {
    pub issues: HealthIssues,
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        self.issues.is_empty()
    }
    
    pub fn print_report(&self) {
        use crate::{println, warn_print, error_print};
        
        if self.is_healthy() {
            println!("Allocator health: GOOD");
            return;
        }
        
        warn_print!("Allocator health issues detected:");
        
        if self.issues.contains(HealthIssues::HIGH_MEMORY_USAGE) {
            warn_print!("  - High memory usage (>90%)");
        }
        
        if self.issues.contains(HealthIssues::HIGH_FRAGMENTATION) {
            warn_print!("  - High fragmentation (>50%)");
        }
        
        if self.issues.contains(HealthIssues::LOW_SUCCESS_RATE) {
            warn_print!("  - Low allocation success rate (<95%)");
        }
        
        if self.issues.contains(HealthIssues::CORRUPTION_DETECTED) {
            error_print!("  - Memory corruption detected!");
        }
        
        if self.issues.contains(HealthIssues::POTENTIAL_LEAK) {
            warn_print!("  - Potential memory leak detected");
        }
    }
}

bitflags::bitflags! {
    /// 健康问题标志
    pub struct HealthIssues: u32 {
        const HIGH_MEMORY_USAGE = 1 << 0;
        const HIGH_FRAGMENTATION = 1 << 1;
        const LOW_SUCCESS_RATE = 1 << 2;
        const CORRUPTION_DETECTED = 1 << 3;
        const POTENTIAL_LEAK = 1 << 4;
    }
}

/// 简单的bitflags实现（因为no_std环境）
mod bitflags {
    macro_rules! bitflags {
        (
            $(#[$outer:meta])*
            pub struct $BitFlags:ident: $T:ty {
                $(
                    $(#[$inner:ident $($args:tt)*])*
                    const $Flag:ident = $value:expr;
                )+
            }
        ) => {
            $(#[$outer])*
            #[derive(Clone, Copy, PartialEq, Eq, Debug)]
            pub struct $BitFlags {
                bits: $T,
            }
            
            impl $BitFlags {
                $(
                    $(#[$inner $($args)*])*
                    pub const $Flag: Self = Self { bits: $value };
                )+
                
                pub const fn empty() -> Self {
                    Self { bits: 0 }
                }
                
                pub const fn is_empty(&self) -> bool {
                    self.bits == 0
                }
                
                pub const fn contains(&self, other: Self) -> bool {
                    (self.bits & other.bits) == other.bits
                }
            }
            
            impl core::ops::BitOr for $BitFlags {
                type Output = Self;
                
                fn bitor(self, other: Self) -> Self {
                    Self { bits: self.bits | other.bits }
                }
            }
            
            impl core::ops::BitOrAssign for $BitFlags {
                fn bitor_assign(&mut self, other: Self) {
                    self.bits |= other.bits;
                }
            }
        };
    }
    
    pub(crate) use bitflags;
}

// 确保块头大小是16字节的倍数，便于对齐
const _: () = {
    assert!(mem::size_of::<BlockHeader>() % 16 == 0,
            "BlockHeader size must be multiple of 16");
};

/// 内存块的迭代器
pub struct BlockIterator {
    current: usize,
    end: usize,
}

impl BlockIterator {
    /// 创建新的块迭代器
    pub fn new(start: usize, end: usize) -> Self {
        Self { current: start, end }
    }
    
    /// 重置迭代器
    pub fn reset(&mut self, start: usize, end: usize) {
        self.current = start;
        self.end = end;
    }
    
    /// 获取当前位置
    pub fn position(&self) -> usize {
        self.current
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
            if (*header).validate() {
                self.current += (*header).total_size();
                Some(header)
            } else {
                // 无效块，停止迭代
                None
            }
        }
    }
}

/// 增强的块验证器
pub struct BlockValidator {
    heap_start: usize,
    heap_end: usize,
    stats: AllocStats,
}

impl BlockValidator {
    /// 创建新的验证器
    pub fn new(heap_start: usize, heap_end: usize) -> Self {
        Self { 
            heap_start, 
            heap_end,
            stats: AllocStats::new(heap_end - heap_start),
        }
    }
    
    /// 验证整个堆的完整性
    pub fn validate_heap(&mut self) -> Result<(), &'static str> {
        let mut current = self.heap_start;
        let mut block_count = 0;
        let mut allocated_count = 0;
        let mut free_count = 0;
        let mut total_allocated_size = 0;
        let mut total_free_size = 0;
        
        while current < self.heap_end {
            let header = current as *const BlockHeader;
            
            unsafe {
                // 检查基本有效性
                if !(*header).validate() {
                    return Err("Block validation failed");
                }
                
                // 检查边界
                let block_end = current + (*header).total_size();
                if block_end > self.heap_end {
                    return Err("Block extends beyond heap boundary");
                }
                
                // 统计信息
                match (*header).status {
                    BlockStatus::Allocated => {
                        allocated_count += 1;
                        total_allocated_size += (*header).size;
                    }
                    BlockStatus::Free => {
                        free_count += 1;
                        total_free_size += (*header).size;
                    }
                }
                
                current = block_end;
                block_count += 1;
            }
            
            // 防止无限循环
            if block_count > 100000 {
                return Err("Too many blocks, possible corruption");
            }
        }
        
        // 确保正好到达堆末尾
        if current != self.heap_end {
            return Err("Heap blocks do not fill entire heap");
        }
        
        // 更新统计信息
        self.stats.alloc_count = allocated_count;
        self.stats.free_count = free_count;
        self.stats.used_size = total_allocated_size;
        self.stats.free_size = total_free_size;
        
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
            // 使用块头的验证方法
            if !(*block).validate() {
                return Err("Block header validation failed");
            }
            
            // 检查块不会超出堆边界
            let block_end = block_addr + (*block).total_size();
            if block_end > self.heap_end {
                return Err("Block extends beyond heap boundary");
            }
        }
        
        Ok(())
    }
    
    /// 获取验证后的统计信息
    pub fn get_stats(&self) -> &AllocStats {
        &self.stats
    }
}

/// 获取时间戳（简化实现）
fn get_timestamp() -> u64 {
    // 在实际系统中，这里会读取硬件计时器
    // 现在使用简单的全局计数器
    static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
    COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
}