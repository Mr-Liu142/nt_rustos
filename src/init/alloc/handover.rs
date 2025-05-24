// 早期分配器接管机制
// 用于将早期分配的内存信息传递给完整的内存管理系统

use super::metadata::AllocStats;
use crate::println;

// 最大可跟踪的已分配块数量（与allocator.rs保持一致）
pub const MAX_TRACKED_BLOCKS: usize = 256;

/// 分配用途枚举
/// 标记内存块的用途，便于内存管理系统接管后进行分类处理
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AllocPurpose {
    Unknown,              // 未知用途
    InterruptTable,       // 中断描述符表
    ProcessControlBlock,  // 进程控制块
    PageTable,           // 页表
    KernelStack,         // 内核栈
    KernelHeap,          // 内核堆
    DriverBuffer,        // 驱动缓冲区
    FileSystemMeta,      // 文件系统元数据
    NetworkBuffer,       // 网络缓冲区
    TempBuffer,          // 临时缓冲区（可回收）
}

impl AllocPurpose {
    /// 判断该用途的内存是否可以被回收
    pub fn is_reclaimable(&self) -> bool {
        match self {
            AllocPurpose::TempBuffer => true,
            AllocPurpose::Unknown => true,  // 未知用途的也可以考虑回收
            _ => false,
        }
    }
    
    /// 判断该用途的内存是否是关键内存
    pub fn is_critical(&self) -> bool {
        match self {
            AllocPurpose::InterruptTable |
            AllocPurpose::ProcessControlBlock |
            AllocPurpose::PageTable |
            AllocPurpose::KernelStack => true,
            _ => false,
        }
    }
    
    /// 获取用途的描述字符串
    pub fn description(&self) -> &'static str {
        match self {
            AllocPurpose::Unknown => "Unknown",
            AllocPurpose::InterruptTable => "Interrupt Table",
            AllocPurpose::ProcessControlBlock => "Process Control Block",
            AllocPurpose::PageTable => "Page Table",
            AllocPurpose::KernelStack => "Kernel Stack",
            AllocPurpose::KernelHeap => "Kernel Heap",
            AllocPurpose::DriverBuffer => "Driver Buffer",
            AllocPurpose::FileSystemMeta => "FileSystem Metadata",
            AllocPurpose::NetworkBuffer => "Network Buffer",
            AllocPurpose::TempBuffer => "Temporary Buffer",
        }
    }
}

/// 已分配块信息
/// 记录一个已分配内存块的详细信息
#[derive(Debug, Clone, Copy)]
pub struct AllocatedBlock {
    /// 内存块起始地址（用户数据地址）
    pub addr: usize,
    
    /// 内存块大小
    pub size: usize,
    
    /// 分配用途
    pub purpose: AllocPurpose,
    
    /// 分配ID（用于追踪）
    pub alloc_id: u64,
}

impl AllocatedBlock {
    /// 获取内存块的结束地址
    pub fn end_addr(&self) -> usize {
        self.addr + self.size
    }
    
    /// 检查地址是否在该块内
    pub fn contains(&self, addr: usize) -> bool {
        addr >= self.addr && addr < self.end_addr()
    }
    
    /// 打印块信息
    pub fn print_info(&self) {
        println!("Block #{}: addr=0x{:x}, size={} bytes, purpose={}", 
                 self.alloc_id, self.addr, self.size, self.purpose.description());
    }
}

/// 接管信息结构
/// 包含早期分配器的所有状态信息，用于传递给内存管理系统
#[derive(Debug, Clone)]
pub struct HandoverInfo {
    /// 堆起始地址
    pub heap_start: usize,
    
    /// 堆结束地址
    pub heap_end: usize,
    
    /// 所有已分配的块（固定大小数组）
    pub allocated_blocks: [AllocatedBlock; MAX_TRACKED_BLOCKS],
    
    /// 实际已分配块的数量
    pub allocated_count: usize,
    
    /// 统计信息
    pub statistics: AllocStats,
}

impl HandoverInfo {
    /// 获取堆大小
    pub fn heap_size(&self) -> usize {
        self.heap_end - self.heap_start
    }
    
    /// 获取已分配块数量
    pub fn allocated_count(&self) -> usize {
        self.allocated_count
    }
    
    /// 获取已分配的总大小
    pub fn allocated_size(&self) -> usize {
        let mut total = 0;
        for i in 0..self.allocated_count {
            total += self.allocated_blocks[i].size;
        }
        total
    }
    
    /// 获取可回收的内存大小
    pub fn reclaimable_size(&self) -> usize {
        let mut total = 0;
        for i in 0..self.allocated_count {
            if self.allocated_blocks[i].purpose.is_reclaimable() {
                total += self.allocated_blocks[i].size;
            }
        }
        total
    }
    
    /// 获取关键内存大小
    pub fn critical_size(&self) -> usize {
        let mut total = 0;
        for i in 0..self.allocated_count {
            if self.allocated_blocks[i].purpose.is_critical() {
                total += self.allocated_blocks[i].size;
            }
        }
        total
    }
    
    /// 按用途分组统计
    pub fn group_by_purpose(&self) -> [(AllocPurpose, usize, usize); 10] {
        let mut groups = [
            (AllocPurpose::Unknown, 0, 0),
            (AllocPurpose::InterruptTable, 0, 0),
            (AllocPurpose::ProcessControlBlock, 0, 0),
            (AllocPurpose::PageTable, 0, 0),
            (AllocPurpose::KernelStack, 0, 0),
            (AllocPurpose::KernelHeap, 0, 0),
            (AllocPurpose::DriverBuffer, 0, 0),
            (AllocPurpose::FileSystemMeta, 0, 0),
            (AllocPurpose::NetworkBuffer, 0, 0),
            (AllocPurpose::TempBuffer, 0, 0),
        ];
        
        for i in 0..self.allocated_count {
            let block = &self.allocated_blocks[i];
            for group in &mut groups {
                if group.0 == block.purpose {
                    group.1 += 1;      // 计数
                    group.2 += block.size;  // 总大小
                    break;
                }
            }
        }
        
        groups
    }
    
    /// 打印接管信息摘要
    pub fn print_summary(&self) {
        println!("=== Early Allocator Handover Summary ===");
        println!("Heap range: 0x{:x} - 0x{:x} ({} KB)", 
                 self.heap_start, self.heap_end, self.heap_size() / 1024);
        println!("Allocated blocks: {}", self.allocated_count());
        println!("Total allocated: {} KB", self.allocated_size() / 1024);
        println!("Reclaimable: {} KB", self.reclaimable_size() / 1024);
        println!("Critical: {} KB", self.critical_size() / 1024);
        
        println!("\nAllocation by purpose:");
        let groups = self.group_by_purpose();
        for (purpose, count, size) in &groups {
            if *count > 0 {
                println!("  {}: {} blocks, {} KB", 
                         purpose.description(), count, size / 1024);
            }
        }
        
        println!("\nStatistics:");
        println!("  Total allocations: {}", self.statistics.total_allocs);
        println!("  Total deallocations: {}", self.statistics.total_frees);
        println!("  Current usage: {}%", self.statistics.usage_percent());
        println!("========================================");
    }
    
    /// 验证接管信息的完整性
    pub fn validate(&self) -> Result<(), &'static str> {
        // 检查堆范围
        if self.heap_start >= self.heap_end {
            return Err("Invalid heap range");
        }
        
        // 检查所有块是否在堆范围内
        for i in 0..self.allocated_count {
            let block = &self.allocated_blocks[i];
            if block.addr < self.heap_start || block.end_addr() > self.heap_end {
                return Err("Block outside heap range");
            }
        }
        
        // 检查块是否重叠
        for i in 0..self.allocated_count {
            for j in (i + 1)..self.allocated_count {
                let block1 = &self.allocated_blocks[i];
                let block2 = &self.allocated_blocks[j];
                
                if block1.addr < block2.end_addr() && block2.addr < block1.end_addr() {
                    return Err("Overlapping blocks detected");
                }
            }
        }
        
        // 检查统计信息一致性
        let calculated_size = self.allocated_size();
        if calculated_size != self.statistics.used_size {
            return Err("Statistics mismatch");
        }
        
        Ok(())
    }
}

/// 接管协议
/// 定义内存管理系统如何接管早期分配器
pub trait HandoverProtocol {
    /// 接收接管信息
    fn receive_handover(&mut self, info: HandoverInfo) -> Result<(), &'static str>;
    
    /// 验证接管信息
    fn validate_handover(&self, info: &HandoverInfo) -> Result<(), &'static str>;
    
    /// 执行接管
    fn execute_handover(&mut self, info: HandoverInfo) -> Result<(), &'static str>;
    
    /// 回收可回收的内存
    fn reclaim_memory(&mut self, blocks: &[AllocatedBlock]) -> usize;
}

/// 接管辅助函数
pub mod handover_utils {
    use super::*;
    
    /// 将已分配块列表转换为内存映射
    /// 返回一个表示内存使用情况的位图（简化版本）
    /// 注意：由于no_std限制，返回固定大小的数组
    pub fn create_memory_map(blocks: &[AllocatedBlock], block_count: usize, heap_start: usize, heap_size: usize) -> [bool; 256] {
        let page_size = 4096;
        let num_pages = (heap_size + page_size - 1) / page_size;
        let mut map = [false; 256]; // 支持最多256个页面（1MB堆/4KB页）
        
        // 确保不会超出数组边界
        let max_pages = num_pages.min(256);
        
        for i in 0..block_count {
            let block = &blocks[i];
            let start_page = (block.addr - heap_start) / page_size;
            let end_page = (block.end_addr() - heap_start + page_size - 1) / page_size;
            
            for page in start_page..end_page.min(max_pages) {
                map[page] = true;
            }
        }
        
        map
    }
    
    /// 查找指定地址所属的块
    pub fn find_block_by_addr(blocks: &[AllocatedBlock], block_count: usize, addr: usize) -> Option<&AllocatedBlock> {
        for i in 0..block_count {
            if blocks[i].contains(addr) {
                return Some(&blocks[i]);
            }
        }
        None
    }
    
    /// 计算内存碎片度
    /// 返回0-100的值，0表示无碎片，100表示严重碎片
    pub fn calculate_fragmentation(blocks: &[AllocatedBlock], block_count: usize, heap_start: usize, heap_end: usize) -> u8 {
        if block_count == 0 {
            return 0;
        }
        
        // 创建一个临时的排序索引数组
        let mut indices = [0usize; MAX_TRACKED_BLOCKS];
        for i in 0..block_count {
            indices[i] = i;
        }
        
        // 简单的冒泡排序（因为no_std环境）
        for i in 0..block_count {
            for j in 0..block_count - 1 - i {
                if blocks[indices[j]].addr > blocks[indices[j + 1]].addr {
                    let temp = indices[j];
                    indices[j] = indices[j + 1];
                    indices[j + 1] = temp;
                }
            }
        }
        
        // 计算空闲间隙数量
        let mut gaps = 0;
        let mut last_end = heap_start;
        
        for i in 0..block_count {
            let block = &blocks[indices[i]];
            if block.addr > last_end {
                gaps += 1;
            }
            last_end = block.end_addr();
        }
        
        // 最后一个块到堆结束的间隙
        if last_end < heap_end {
            gaps += 1;
        }
        
        // 碎片度 = (间隙数 / 块数) * 100
        let fragmentation = (gaps as f32 / block_count as f32) * 100.0;
        fragmentation.min(100.0) as u8
    }
}