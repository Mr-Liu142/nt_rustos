// 生产级早期分配器元数据管理
// 定义完善的块头、统计信息等数据结构

use super::handover::AllocPurpose;
use core::mem;

// 块头魔数
pub const BLOCK_MAGIC: u32 = 0xB10C4EA0; // BLOCK HEAD

/// 块状态枚举
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
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
            checksum: 0, // 校验和初始为0
            #[cfg(target_pointer_width = "64")]
            padding: [0; 4],
            #[cfg(target_pointer_width = "32")]
            padding: [0; 8],
        };
        
        // 基于其他字段的值计算并填充校验和
        header.update_checksum();
        header
    }
    
    /// 验证块头完整性
    pub fn validate(&self) -> bool {
        if self.magic != BLOCK_MAGIC {
            return false;
        }
        
        // 验证校验和
        if self.checksum != self.calculate_checksum() {
            return false;
        }
        
        true
    }
    
    /// 计算校验和 (修正版：独立于checksum字段自身)
    fn calculate_checksum(&self) -> u32 {
        // 使用一个简单的 wrapping add 算法
        let mut checksum = self.size as u32;
        checksum = checksum.wrapping_add((self.size >> 32) as u32);
        checksum = checksum.wrapping_add(self.status as u32);
        checksum = checksum.wrapping_add(self.magic);
        checksum = checksum.wrapping_add(self.alloc_id as u32);
        checksum = checksum.wrapping_add((self.alloc_id >> 32) as u32);
        checksum = checksum.wrapping_add(self.purpose as u32);
        checksum = checksum.wrapping_add(self.timestamp as u32);
        checksum = checksum.wrapping_add((self.timestamp >> 32) as u32);
        // 注意：这里没有包含 self.checksum 自身
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
    pub total_size: usize,
    pub used_size: usize,
    pub free_size: usize,
    pub alloc_count: usize,
    pub free_count: usize,
    pub total_allocs: u64,
    pub total_frees: u64,
    pub failed_allocs: u64,
    pub double_free_attempts: u64,
    pub corrupted_blocks: u64,
    pub max_alloc_size: usize,
    pub min_alloc_size: usize,
    pub avg_alloc_size: usize,
    pub merge_count: u64,
    pub split_count: u64,
    pub coalesce_count: u64,
    pub peak_used_size: usize,
    pub max_free_block_size: usize,
    pub fragmentation_percent: u8,
}

impl AllocStats {
    pub fn new(total_size: usize) -> Self {
        Self {
            total_size,
            used_size: 0,
            free_size: total_size,
            alloc_count: 0,
            free_count: 1,
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
    
    pub fn record_alloc(&mut self, size: usize) {
        self.used_size += size;
        self.total_allocs += 1;
        self.alloc_count += 1;
        self.max_alloc_size = self.max_alloc_size.max(size);
        self.min_alloc_size = self.min_alloc_size.min(size);
        self.peak_used_size = self.peak_used_size.max(self.used_size);
        if self.total_allocs > 0 {
            self.avg_alloc_size = (self.used_size as u64 / self.total_allocs) as usize;
        }
    }
    
    pub fn record_dealloc(&mut self, size: usize) {
        self.used_size -= size;
        self.total_frees += 1;
        self.alloc_count = self.alloc_count.saturating_sub(1);
    }

    pub fn record_merge(&mut self) {
        self.merge_count += 1;
        self.coalesce_count += 1;
    }

    pub fn record_split(&mut self, _new_free_size: usize) {
        self.split_count += 1;
    }
    
    pub fn record_alloc_failure(&mut self) { self.failed_allocs += 1; }
    pub fn record_double_free(&mut self) { self.double_free_attempts += 1; }
    pub fn record_corruption(&mut self) { self.corrupted_blocks += 1; }
    
    pub fn usage_percent(&self) -> u8 {
        if self.total_size == 0 { return 0; }
        ((self.used_size as f32 / self.total_size as f32) * 100.0) as u8
    }
    
    pub fn fragmentation_estimate(&self) -> u8 {
        if self.free_size == 0 || self.max_free_block_size == 0 { return 0; }
        (100.0 - (self.max_free_block_size as f32 / self.free_size as f32) * 100.0) as u8
    }
    
    pub fn success_rate(&self) -> u8 {
        let total_attempts = self.total_allocs + self.failed_allocs;
        if total_attempts == 0 { return 100; }
        ((self.total_allocs as f32 / total_attempts as f32) * 100.0) as u8
    }
    
    pub fn print_detailed(&self) {
        use crate::println;
        println!("=== Detailed Memory Statistics ===");
        println!("Memory Layout:");
        println!("  Total size: {} KB", self.total_size / 1024);
        println!("  Used: {} KB ({}%)", self.used_size / 1024, self.usage_percent());
        println!("  Free: {} KB", self.free_size / 1024);
        println!("  Peak usage: {} KB", self.peak_used_size / 1024);
        println!("Block Statistics:");
        println!("  Allocated blocks: {}", self.alloc_count);
        println!("  Free blocks: {}", self.free_count);
        println!("  Max free block: {} KB", self.max_free_block_size / 1024);
        println!("Allocation Statistics:");
        println!("  Total allocations: {}", self.total_allocs);
        println!("  Total deallocations: {}", self.total_frees);
        println!("  Failed allocations: {}", self.failed_allocs);
        println!("  Success rate: {}%", self.success_rate());
        println!("Size Statistics:");
        if self.min_alloc_size != usize::MAX { println!("  Min allocation: {} bytes", self.min_alloc_size); }
        println!("  Max allocation: {} bytes", self.max_alloc_size);
        println!("  Average allocation: {} bytes", self.avg_alloc_size);
        println!("Performance Statistics:");
        println!("  Block merges: {}", self.merge_count);
        println!("  Block splits: {}", self.split_count);
        println!("  Coalesce operations: {}", self.coalesce_count);
        println!("  Fragmentation: {}%", self.fragmentation_estimate());
        println!("Error Statistics:");
        println!("  Double free attempts: {}", self.double_free_attempts);
        println!("  Corrupted blocks: {}", self.corrupted_blocks);
        println!("=====================================");
    }
    
    pub fn print_summary(&self) {
        use crate::println;
        println!("Memory: {}/{} KB ({}%), Allocs: {}/{}, Frag: {}%",
                 self.used_size / 1024, self.total_size / 1024, self.usage_percent(),
                 self.total_frees, self.total_allocs, self.fragmentation_estimate());
    }
    
    pub fn check_health(&self) -> HealthStatus {
        let mut issues = HealthIssues::empty();
        if self.usage_percent() > 90 { issues |= HealthIssues::HIGH_MEMORY_USAGE; }
        if self.fragmentation_estimate() > 50 { issues |= HealthIssues::HIGH_FRAGMENTATION; }
        if self.success_rate() < 95 && self.total_allocs > 100 { issues |= HealthIssues::LOW_SUCCESS_RATE; }
        if self.double_free_attempts > 0 || self.corrupted_blocks > 0 { issues |= HealthIssues::CORRUPTION_DETECTED; }
        if self.total_allocs > self.total_frees && (self.total_allocs - self.total_frees) > 1000 {
            issues |= HealthIssues::POTENTIAL_LEAK;
        }
        HealthStatus { issues }
    }
}

pub struct HealthStatus { pub issues: HealthIssues, }
impl HealthStatus {
    pub fn is_healthy(&self) -> bool { self.issues.is_empty() }
    pub fn print_report(&self) {
        use crate::{println, warn_print, error_print};
        if self.is_healthy() { println!("Allocator health: GOOD"); return; }
        warn_print!("Allocator health issues detected:");
        if self.issues.contains(HealthIssues::HIGH_MEMORY_USAGE) { warn_print!("  - High memory usage (>90%)"); }
        if self.issues.contains(HealthIssues::HIGH_FRAGMENTATION) { warn_print!("  - High fragmentation (>50%)"); }
        if self.issues.contains(HealthIssues::LOW_SUCCESS_RATE) { warn_print!("  - Low allocation success rate (<95%)"); }
        if self.issues.contains(HealthIssues::CORRUPTION_DETECTED) { error_print!("  - Memory corruption detected!"); }
        if self.issues.contains(HealthIssues::POTENTIAL_LEAK) { warn_print!("  - Potential memory leak detected"); }
    }
}

mod bitflags {
    macro_rules! bitflags {
        ($(#[$outer:meta])* pub struct $BitFlags:ident: $T:ty { $($(#[$inner:ident $($args:tt)*])* const $Flag:ident = $value:expr;)+ }) => {
            $(#[$outer])* #[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct $BitFlags { bits: $T, }
            impl $BitFlags {
                $( $(#[$inner $($args)*])* pub const $Flag: Self = Self { bits: $value }; )+
                pub const fn empty() -> Self { Self { bits: 0 } }
                pub const fn is_empty(&self) -> bool { self.bits == 0 }
                pub const fn contains(&self, other: Self) -> bool { (self.bits & other.bits) == other.bits }
            }
            impl core::ops::BitOr for $BitFlags { type Output = Self; fn bitor(self, other: Self) -> Self { Self { bits: self.bits | other.bits } } }
            impl core::ops::BitOrAssign for $BitFlags { fn bitor_assign(&mut self, other: Self) { self.bits |= other.bits; } }
        };
    }
    pub(crate) use bitflags;
}

bitflags::bitflags! {
    pub struct HealthIssues: u32 {
        const HIGH_MEMORY_USAGE = 1 << 0;
        const HIGH_FRAGMENTATION = 1 << 1;
        const LOW_SUCCESS_RATE = 1 << 2;
        const CORRUPTION_DETECTED = 1 << 3;
        const POTENTIAL_LEAK = 1 << 4;
    }
}

const _: () = { assert!(mem::size_of::<BlockHeader>() % 16 == 0, "BlockHeader size must be multiple of 16"); };

pub struct BlockIterator { current: usize, end: usize, }
impl BlockIterator {
    pub fn new(start: usize, end: usize) -> Self { Self { current: start, end } }
}
impl Iterator for BlockIterator {
    type Item = *const BlockHeader;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.end { return None; }
        let header = self.current as *const BlockHeader;
        unsafe {
            if (*header).magic == BLOCK_MAGIC {
                self.current += (*header).total_size();
                Some(header)
            } else { None }
        }
    }
}

pub struct BlockValidator { heap_start: usize, heap_end: usize, stats: AllocStats, }
impl BlockValidator {
    pub fn new(heap_start: usize, heap_end: usize) -> Self { Self { heap_start, heap_end, stats: AllocStats::new(heap_end - heap_start), } }
    pub fn validate_heap(&mut self) -> Result<(), &'static str> {
        let mut iter = BlockIterator::new(self.heap_start, self.heap_end);
        let mut total_size = 0;
        while let Some(header) = iter.next() {
            unsafe {
                if !(*header).validate() { return Err("Block validation failed"); }
                if (*header).user_data_addr() + (*header).size > self.heap_end { return Err("Block extends beyond heap boundary"); }
                total_size += (*header).total_size();
            }
        }
        if total_size != self.heap_end - self.heap_start { return Err("Heap blocks do not fill entire heap"); }
        Ok(())
    }
}

fn get_timestamp() -> u64 {
    static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
    COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
}