// 生产级早期分配器接管机制
// 用于将早期分配的内存信息安全传递给完整的内存管理系统

use super::metadata::AllocStats;
use crate::{println, warn_print, error_print, info_print};

// 最大可跟踪的已分配块数量
pub const MAX_TRACKED_BLOCKS: usize = 512;

// 接管协议版本
pub const HANDOVER_PROTOCOL_VERSION: u32 = 1;

// 接管魔数
pub const HANDOVER_MAGIC: u64 = 0x48414E444F564552; // "HANDOVER"

/// 分配用途枚举 - 扩展版本
/// 标记内存块的用途，便于内存管理系统接管后进行分类处理
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum AllocPurpose {
    Unknown = 0,              // 未知用途
    InterruptTable = 1,       // 中断描述符表
    ProcessControlBlock = 2,  // 进程控制块
    PageTable = 3,           // 页表
    KernelStack = 4,         // 内核栈
    KernelHeap = 5,          // 内核堆
    DriverBuffer = 6,        // 驱动缓冲区
    FileSystemMeta = 7,      // 文件系统元数据
    NetworkBuffer = 8,       // 网络缓冲区
    TempBuffer = 9,          // 临时缓冲区（可回收）
    BootstrapData = 10,      // 引导数据
    DeviceTree = 11,         // 设备树
    SymbolTable = 12,        // 符号表
    ModuleCode = 13,         // 模块代码
    CacheBuffer = 14,        // 缓存缓冲区
    SharedMemory = 15,       // 共享内存
    UserData = 16,           // 用户数据
    SystemCall = 17,         // 系统调用相关
    Debugging = 18,          // 调试信息
    Testing = 19,            // 测试数据
}

impl AllocPurpose {
    /// 判断该用途的内存是否可以被回收
    pub fn is_reclaimable(&self) -> bool {
        match self {
            AllocPurpose::TempBuffer |
            AllocPurpose::CacheBuffer |
            AllocPurpose::Testing |
            AllocPurpose::Unknown => true,
            _ => false,
        }
    }
    
    /// 判断该用途的内存是否是关键内存
    pub fn is_critical(&self) -> bool {
        match self {
            AllocPurpose::InterruptTable |
            AllocPurpose::ProcessControlBlock |
            AllocPurpose::PageTable |
            AllocPurpose::KernelStack |
            AllocPurpose::BootstrapData |
            AllocPurpose::DeviceTree => true,
            _ => false,
        }
    }
    
    /// 判断该用途的内存是否可以被移动
    pub fn is_movable(&self) -> bool {
        match self {
            AllocPurpose::TempBuffer |
            AllocPurpose::CacheBuffer |
            AllocPurpose::UserData |
            AllocPurpose::Testing => true,
            _ => false,
        }
    }
    
    /// 判断该用途的内存是否需要特殊对齐
    pub fn requires_special_alignment(&self) -> bool {
        match self {
            AllocPurpose::PageTable |
            AllocPurpose::InterruptTable |
            AllocPurpose::DeviceTree => true,
            _ => false,
        }
    }
    
    /// 获取推荐的对齐大小
    pub fn recommended_alignment(&self) -> usize {
        match self {
            AllocPurpose::PageTable => 4096,      // 页对齐
            AllocPurpose::InterruptTable => 256,  // 中断表对齐
            AllocPurpose::DeviceTree => 8,        // 设备树对齐
            _ => 8,                               // 默认对齐
        }
    }
    
    /// 获取用途的优先级（0最高，255最低）
    pub fn priority(&self) -> u8 {
        match self {
            AllocPurpose::InterruptTable => 0,
            AllocPurpose::PageTable => 1,
            AllocPurpose::ProcessControlBlock => 2,
            AllocPurpose::KernelStack => 3,
            AllocPurpose::BootstrapData => 4,
            AllocPurpose::DeviceTree => 5,
            AllocPurpose::KernelHeap => 10,
            AllocPurpose::DriverBuffer => 20,
            AllocPurpose::SystemCall => 30,
            AllocPurpose::ModuleCode => 40,
            AllocPurpose::SymbolTable => 50,
            AllocPurpose::FileSystemMeta => 60,
            AllocPurpose::NetworkBuffer => 70,
            AllocPurpose::SharedMemory => 80,
            AllocPurpose::UserData => 90,
            AllocPurpose::CacheBuffer => 100,
            AllocPurpose::Debugging => 200,
            AllocPurpose::TempBuffer => 240,
            AllocPurpose::Testing => 250,
            AllocPurpose::Unknown => 255,
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
            AllocPurpose::BootstrapData => "Bootstrap Data",
            AllocPurpose::DeviceTree => "Device Tree",
            AllocPurpose::SymbolTable => "Symbol Table",
            AllocPurpose::ModuleCode => "Module Code",
            AllocPurpose::CacheBuffer => "Cache Buffer",
            AllocPurpose::SharedMemory => "Shared Memory",
            AllocPurpose::UserData => "User Data",
            AllocPurpose::SystemCall => "System Call",
            AllocPurpose::Debugging => "Debugging Info",
            AllocPurpose::Testing => "Testing Data",
        }
    }
    
    /// 获取简短标识符
    pub fn short_name(&self) -> &'static str {
        match self {
            AllocPurpose::Unknown => "UNK",
            AllocPurpose::InterruptTable => "INT",
            AllocPurpose::ProcessControlBlock => "PCB",
            AllocPurpose::PageTable => "PGT",
            AllocPurpose::KernelStack => "KST",
            AllocPurpose::KernelHeap => "KHP",
            AllocPurpose::DriverBuffer => "DRV",
            AllocPurpose::FileSystemMeta => "FSM",
            AllocPurpose::NetworkBuffer => "NET",
            AllocPurpose::TempBuffer => "TMP",
            AllocPurpose::BootstrapData => "BST",
            AllocPurpose::DeviceTree => "DTB",
            AllocPurpose::SymbolTable => "SYM",
            AllocPurpose::ModuleCode => "MOD",
            AllocPurpose::CacheBuffer => "CHE",
            AllocPurpose::SharedMemory => "SHM",
            AllocPurpose::UserData => "USR",
            AllocPurpose::SystemCall => "SYS",
            AllocPurpose::Debugging => "DBG",
            AllocPurpose::Testing => "TST",
        }
    }
}

/// 已分配块信息 - 增强版本
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
    
    /// 分配时间戳
    pub timestamp: u64,
    
    /// 访问权限标志
    pub permissions: MemoryPermissions,
    
    /// 对齐要求
    pub alignment: usize,
    
    /// 保留字段，用于未来扩展
    pub reserved: [u32; 2],
}

impl AllocatedBlock {
    /// 创建新的已分配块信息
    pub fn new(addr: usize, size: usize, purpose: AllocPurpose, alloc_id: u64) -> Self {
        Self {
            addr,
            size,
            purpose,
            alloc_id,
            timestamp: get_timestamp(),
            permissions: MemoryPermissions::READ_WRITE,
            alignment: 8,
            reserved: [0; 2],
        }
    }
    
    /// 获取内存块的结束地址
    pub fn end_addr(&self) -> usize {
        self.addr + self.size
    }
    
    /// 检查地址是否在该块内
    pub fn contains(&self, addr: usize) -> bool {
        addr >= self.addr && addr < self.end_addr()
    }
    
    /// 检查是否与另一个块重叠
    pub fn overlaps_with(&self, other: &AllocatedBlock) -> bool {
        self.addr < other.end_addr() && other.addr < self.end_addr()
    }
    
    /// 获取块的年龄（相对时间）
    pub fn age(&self) -> u64 {
        get_timestamp().saturating_sub(self.timestamp)
    }
    
    /// 检查是否是古老的块
    pub fn is_old(&self, threshold: u64) -> bool {
        self.age() > threshold
    }
    
    /// 打印块信息
    pub fn print_info(&self) {
        println!("Block #{}: addr=0x{:x}, size={} bytes, purpose={} ({}), age={}", 
                 self.alloc_id, self.addr, self.size, 
                 self.purpose.short_name(), self.purpose.description(), self.age());
    }
    
    /// 打印详细块信息
    pub fn print_detailed(&self) {
        println!("=== Block Details ===");
        println!("ID: {}", self.alloc_id);
        println!("Address: 0x{:x} - 0x{:x}", self.addr, self.end_addr());
        println!("Size: {} bytes ({} KB)", self.size, self.size / 1024);
        println!("Purpose: {} ({})", self.purpose.description(), self.purpose.short_name());
        println!("Priority: {}", self.purpose.priority());
        println!("Alignment: {} bytes", self.alignment);
        println!("Permissions: {:?}", self.permissions);
        println!("Age: {} ticks", self.age());
        println!("Critical: {}", self.purpose.is_critical());
        println!("Reclaimable: {}", self.purpose.is_reclaimable());
        println!("Movable: {}", self.purpose.is_movable());
        println!("===================");
    }
}

/// 内存权限标志
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryPermissions {
    bits: u8,
}

impl MemoryPermissions {
    pub const READ: Self = Self { bits: 1 << 0 };
    pub const WRITE: Self = Self { bits: 1 << 1 };
    pub const EXECUTE: Self = Self { bits: 1 << 2 };
    pub const USER: Self = Self { bits: 1 << 3 };
    pub const CACHED: Self = Self { bits: 1 << 4 };
    pub const SHARED: Self = Self { bits: 1 << 5 };
    
    pub const READ_WRITE: Self = Self { bits: Self::READ.bits | Self::WRITE.bits };
    pub const READ_EXECUTE: Self = Self { bits: Self::READ.bits | Self::EXECUTE.bits };
    pub const READ_WRITE_EXECUTE: Self = Self { 
        bits: Self::READ.bits | Self::WRITE.bits | Self::EXECUTE.bits 
    };
    
    pub fn contains(&self, other: Self) -> bool {
        (self.bits & other.bits) == other.bits
    }
    
    pub fn can_read(&self) -> bool {
        self.contains(Self::READ)
    }
    
    pub fn can_write(&self) -> bool {
        self.contains(Self::WRITE)
    }
    
    pub fn can_execute(&self) -> bool {
        self.contains(Self::EXECUTE)
    }
}

/// 接管信息结构 - 增强版本
/// 包含早期分配器的所有状态信息，用于传递给内存管理系统
#[derive(Debug, Clone)]
pub struct HandoverInfo {
    /// 协议版本
    pub version: u32,
    
    /// 魔数
    pub magic: u64,
    
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
    
    /// 分配器状态快照
    pub allocator_state: AllocatorState,
    
    /// 接管时间戳
    pub handover_timestamp: u64,
    
    /// 校验和
    pub checksum: u32,
}

/// 分配器状态快照
#[derive(Debug, Clone)]
pub struct AllocatorState {
    /// 是否已冻结
    pub frozen: bool,
    
    /// 完整性检查结果
    pub integrity_ok: bool,
    
    /// 最后的健康检查状态
    pub health_status: u8,
    
    /// 运行时错误计数
    pub error_count: u32,
    
    /// 性能指标
    pub performance_metrics: PerformanceMetrics,
}

/// 性能指标
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// 平均分配时间（假设的时间单位）
    pub avg_alloc_time: u32,
    
    /// 平均释放时间
    pub avg_dealloc_time: u32,
    
    /// 缓存命中率（百分比）
    pub cache_hit_rate: u8,
    
    /// 碎片整理次数
    pub defrag_count: u32,
    
    /// 最大连续分配失败次数
    pub max_consecutive_failures: u32,
}

impl HandoverInfo {
    /// 创建新的接管信息
    pub fn new(heap_start: usize, heap_end: usize, stats: AllocStats) -> Self {
        let mut info = Self {
            version: HANDOVER_PROTOCOL_VERSION,
            magic: HANDOVER_MAGIC,
            heap_start,
            heap_end,
            allocated_blocks: [AllocatedBlock {
                addr: 0,
                size: 0,
                purpose: AllocPurpose::Unknown,
                alloc_id: 0,
                timestamp: 0,
                permissions: MemoryPermissions::READ_WRITE,
                alignment: 8,
                reserved: [0; 2],
            }; MAX_TRACKED_BLOCKS],
            allocated_count: 0,
            statistics: stats,
            allocator_state: AllocatorState {
                frozen: false,
                integrity_ok: true,
                health_status: 0,
                error_count: 0,
                performance_metrics: PerformanceMetrics {
                    avg_alloc_time: 0,
                    avg_dealloc_time: 0,
                    cache_hit_rate: 100,
                    defrag_count: 0,
                    max_consecutive_failures: 0,
                },
            },
            handover_timestamp: get_timestamp(),
            checksum: 0,
        };
        
        info.update_checksum();
        info
    }
    
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
    
    /// 获取可移动内存大小
    pub fn movable_size(&self) -> usize {
        let mut total = 0;
        for i in 0..self.allocated_count {
            if self.allocated_blocks[i].purpose.is_movable() {
                total += self.allocated_blocks[i].size;
            }
        }
        total
    }
    
    /// 按用途分组统计 - 扩展版本
    pub fn group_by_purpose(&self) -> [(AllocPurpose, usize, usize); 20] {
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
            (AllocPurpose::BootstrapData, 0, 0),
            (AllocPurpose::DeviceTree, 0, 0),
            (AllocPurpose::SymbolTable, 0, 0),
            (AllocPurpose::ModuleCode, 0, 0),
            (AllocPurpose::CacheBuffer, 0, 0),
            (AllocPurpose::SharedMemory, 0, 0),
            (AllocPurpose::UserData, 0, 0),
            (AllocPurpose::SystemCall, 0, 0),
            (AllocPurpose::Debugging, 0, 0),
            (AllocPurpose::Testing, 0, 0),
        ];
        
        for i in 0..self.allocated_count {
            let block = &self.allocated_blocks[i];
            for group in &mut groups {
                if group.0 as u8 == block.purpose as u8 {
                    group.1 += 1;      // 计数
                    group.2 += block.size;  // 总大小
                    break;
                }
            }
        }
        
        groups
    }
    
    /// 按优先级排序的块列表
    pub fn blocks_by_priority(&self) -> [usize; MAX_TRACKED_BLOCKS] {
        let mut indices = [0usize; MAX_TRACKED_BLOCKS];
        
        // 初始化索引
        for i in 0..self.allocated_count {
            indices[i] = i;
        }
        
        // 简单的冒泡排序（按优先级）
        for i in 0..self.allocated_count {
            for j in 0..self.allocated_count - 1 - i {
                let priority_j = self.allocated_blocks[indices[j]].purpose.priority();
                let priority_j1 = self.allocated_blocks[indices[j + 1]].purpose.priority();
                
                if priority_j > priority_j1 {
                    let temp = indices[j];
                    indices[j] = indices[j + 1];
                    indices[j + 1] = temp;
                }
            }
        }
        
        indices
    }
    
    /// 查找古老的块
    pub fn find_old_blocks(&self, age_threshold: u64) -> [usize; MAX_TRACKED_BLOCKS] {
        let mut old_blocks = [usize::MAX; MAX_TRACKED_BLOCKS];
        let mut count = 0;
        
        for i in 0..self.allocated_count {
            if self.allocated_blocks[i].is_old(age_threshold) && count < MAX_TRACKED_BLOCKS {
                old_blocks[count] = i;
                count += 1;
            }
        }
        
        old_blocks
    }
    
    /// 检测内存泄漏的可能性
    pub fn detect_potential_leaks(&self) -> LeakDetectionResult {
        let mut result = LeakDetectionResult {
            suspicious_blocks: [usize::MAX; 64],
            suspicious_count: 0,
            total_suspicious_size: 0,
            oldest_block_age: 0,
            leak_score: 0,
        };
        
        let age_threshold = 10000; // 假设的阈值
        let size_threshold = 1024 * 1024; // 1MB
        
        for i in 0..self.allocated_count {
            let block = &self.allocated_blocks[i];
            let mut suspicious = false;
            
            // 检查古老的块
            if block.is_old(age_threshold) {
                suspicious = true;
                result.oldest_block_age = result.oldest_block_age.max(block.age());
            }
            
            // 检查大块
            if block.size > size_threshold {
                suspicious = true;
            }
            
            // 检查临时或测试数据
            if matches!(block.purpose, AllocPurpose::TempBuffer | AllocPurpose::Testing) 
               && block.is_old(1000) {
                suspicious = true;
            }
            
            if suspicious && result.suspicious_count < 64 {
                result.suspicious_blocks[result.suspicious_count] = i;
                result.suspicious_count += 1;
                result.total_suspicious_size += block.size;
            }
        }
        
        // 计算泄漏分数
        result.leak_score = (result.suspicious_count as f32 / self.allocated_count.max(1) as f32 * 100.0) as u8;
        
        result
    }
    
    /// 计算校验和
    fn calculate_checksum(&self) -> u32 {
        let mut checksum = 0u32;
        
        checksum = checksum.wrapping_add(self.version);
        checksum = checksum.wrapping_add(self.magic as u32);
        checksum = checksum.wrapping_add((self.magic >> 32) as u32);
        checksum = checksum.wrapping_add(self.heap_start as u32);
        checksum = checksum.wrapping_add(self.heap_end as u32);
        checksum = checksum.wrapping_add(self.allocated_count as u32);
        
        // 加入部分块的信息以避免过度计算
        for i in 0..self.allocated_count.min(16) {
            let block = &self.allocated_blocks[i];
            checksum = checksum.wrapping_add(block.addr as u32);
            checksum = checksum.wrapping_add(block.size as u32);
            checksum = checksum.wrapping_add(block.alloc_id as u32);
        }
        
        checksum
    }
    
    /// 更新校验和
    pub fn update_checksum(&mut self) {
        self.checksum = self.calculate_checksum();
    }
    
    /// 打印接管信息摘要
    pub fn print_summary(&self) {
        println!("=== Early Allocator Handover Summary ===");
        println!("Protocol Version: {}", self.version);
        println!("Heap range: 0x{:x} - 0x{:x} ({} KB)", 
                 self.heap_start, self.heap_end, self.heap_size() / 1024);
        println!("Allocated blocks: {}/{}", self.allocated_count(), MAX_TRACKED_BLOCKS);
        println!("Total allocated: {} KB", self.allocated_size() / 1024);
        println!("Critical memory: {} KB", self.critical_size() / 1024);
        println!("Reclaimable memory: {} KB", self.reclaimable_size() / 1024);
        println!("Movable memory: {} KB", self.movable_size() / 1024);
        
        println!("\nAllocation by purpose:");
        let groups = self.group_by_purpose();
        for (purpose, count, size) in &groups {
            if *count > 0 {
                println!("  {}: {} blocks, {} KB (priority: {})", 
                         purpose.description(), count, size / 1024, purpose.priority());
            }
        }
        
        println!("\nAllocator State:");
        println!("  Frozen: {}", self.allocator_state.frozen);
        println!("  Integrity: {}", if self.allocator_state.integrity_ok { "OK" } else { "FAILED" });
        println!("  Error count: {}", self.allocator_state.error_count);
        
        println!("\nPerformance Metrics:");
        let perf = &self.allocator_state.performance_metrics;
        println!("  Cache hit rate: {}%", perf.cache_hit_rate);
        println!("  Defragmentation count: {}", perf.defrag_count);
        println!("  Max consecutive failures: {}", perf.max_consecutive_failures);
        
        println!("\nStatistics:");
        println!("  Total allocations: {}", self.statistics.total_allocs);
        println!("  Total deallocations: {}", self.statistics.total_frees);
        println!("  Current usage: {}%", self.statistics.usage_percent());
        println!("  Fragmentation: {}%", self.statistics.fragmentation_estimate());
        println!("========================================");
    }
    
    /// 打印详细报告
    pub fn print_detailed_report(&self) {
        self.print_summary();
        
        println!("\n=== Detailed Block Analysis ===");
        
        // 按优先级显示块
        let priority_indices = self.blocks_by_priority();
        println!("Blocks by priority (highest first):");
        for i in 0..self.allocated_count.min(10) {
            let block_idx = priority_indices[i];
            let block = &self.allocated_blocks[block_idx];
            println!("  #{}: {} - 0x{:x} ({} KB) - {}", 
                     block.alloc_id, block.purpose.short_name(),
                     block.addr, block.size / 1024, block.purpose.description());
        }
        
        // 泄漏检测
        let leak_result = self.detect_potential_leaks();
        if leak_result.suspicious_count > 0 {
            warn_print!("Potential memory leaks detected!");
            println!("  Suspicious blocks: {}", leak_result.suspicious_count);
            println!("  Suspicious size: {} KB", leak_result.total_suspicious_size / 1024);
            println!("  Leak score: {}%", leak_result.leak_score);
            println!("  Oldest block age: {} ticks", leak_result.oldest_block_age);
        }
        
        println!("===============================");
    }
    
    /// 验证接管信息的完整性
    pub fn validate(&self) -> Result<(), &'static str> {
        // 检查魔数和版本
        if self.magic != HANDOVER_MAGIC {
            return Err("Invalid handover magic");
        }
        
        if self.version != HANDOVER_PROTOCOL_VERSION {
            return Err("Unsupported protocol version");
        }
        
        // 检查堆范围
        if self.heap_start >= self.heap_end {
            return Err("Invalid heap range");
        }
        
        // 检查块数量
        if self.allocated_count > MAX_TRACKED_BLOCKS {
            return Err("Too many allocated blocks");
        }
        
        // 检查校验和
        let calculated_checksum = self.calculate_checksum();
        if self.checksum != calculated_checksum {
            return Err("Checksum validation failed");
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
                
                if block1.overlaps_with(block2) {
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

/// 泄漏检测结果
#[derive(Debug)]
pub struct LeakDetectionResult {
    /// 可疑块的索引
    pub suspicious_blocks: [usize; 64],
    
    /// 可疑块数量
    pub suspicious_count: usize,
    
    /// 可疑块总大小
    pub total_suspicious_size: usize,
    
    /// 最古老块的年龄
    pub oldest_block_age: u64,
    
    /// 泄漏分数（0-100）
    pub leak_score: u8,
}

/// 接管协议特征
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
    
    /// 重新定位可移动的内存
    fn relocate_memory(&mut self, blocks: &[AllocatedBlock]) -> Result<(), &'static str>;
    
    /// 升级内存保护
    fn upgrade_protection(&mut self, blocks: &[AllocatedBlock]) -> Result<(), &'static str>;
}

/// 接管辅助函数
pub mod handover_utils {
    use super::*;
    
    /// 将已分配块列表转换为内存映射
    pub fn create_memory_map(
        blocks: &[AllocatedBlock], 
        block_count: usize, 
        heap_start: usize, 
        heap_size: usize
    ) -> [MemoryMapEntry; 512] {
        let page_size = 4096;
        let num_pages = (heap_size + page_size - 1) / page_size;
        let mut map = [MemoryMapEntry::free(); 512];
        
        let max_pages = num_pages.min(512);
        
        for i in 0..block_count {
            let block = &blocks[i];
            let start_page = (block.addr - heap_start) / page_size;
            let end_page = (block.end_addr() - heap_start + page_size - 1) / page_size;
            
            for page in start_page..end_page.min(max_pages) {
                map[page] = MemoryMapEntry::occupied(block.purpose, block.alloc_id);
            }
        }
        
        map
    }
    
    /// 内存映射条目
    #[derive(Debug, Clone, Copy)]
    pub struct MemoryMapEntry {
        pub status: PageStatus,
        pub purpose: AllocPurpose,
        pub alloc_id: u64,
    }
    
    impl MemoryMapEntry {
        pub fn free() -> Self {
            Self {
                status: PageStatus::Free,
                purpose: AllocPurpose::Unknown,
                alloc_id: 0,
            }
        }
        
        pub fn occupied(purpose: AllocPurpose, alloc_id: u64) -> Self {
            Self {
                status: PageStatus::Occupied,
                purpose,
                alloc_id,
            }
        }
    }
    
    /// 页面状态
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum PageStatus {
        Free,
        Occupied,
        Reserved,
    }
    
    /// 查找指定地址所属的块
    pub fn find_block_by_addr(
        blocks: &[AllocatedBlock], 
        block_count: usize, 
        addr: usize
    ) -> Option<&AllocatedBlock> {
        for i in 0..block_count {
            if blocks[i].contains(addr) {
                return Some(&blocks[i]);
            }
        }
        None
    }
    
    /// 查找指定用途的所有块
    pub fn find_blocks_by_purpose(
        blocks: &[AllocatedBlock], 
        block_count: usize, 
        purpose: AllocPurpose
    ) -> [usize; MAX_TRACKED_BLOCKS] {
        let mut result = [usize::MAX; MAX_TRACKED_BLOCKS];
        let mut count = 0;
        
        for i in 0..block_count {
            if blocks[i].purpose as u8 == purpose as u8 && count < MAX_TRACKED_BLOCKS {
                result[count] = i;
                count += 1;
            }
        }
        
        result
    }
    
    /// 计算高级碎片度
    pub fn calculate_advanced_fragmentation(
        blocks: &[AllocatedBlock], 
        block_count: usize, 
        heap_start: usize, 
        heap_end: usize
    ) -> FragmentationAnalysis {
        let mut analysis = FragmentationAnalysis {
            external_fragmentation: 0,
            internal_fragmentation: 0,
            largest_free_block: 0,
            free_block_count: 0,
            fragmentation_score: 0,
        };
        
        // 创建排序后的块列表
        let mut sorted_blocks = [0usize; MAX_TRACKED_BLOCKS];
        for i in 0..block_count {
            sorted_blocks[i] = i;
        }
        
        // 简单排序
        for i in 0..block_count {
            for j in 0..block_count - 1 - i {
                if blocks[sorted_blocks[j]].addr > blocks[sorted_blocks[j + 1]].addr {
                    let temp = sorted_blocks[j];
                    sorted_blocks[j] = sorted_blocks[j + 1];
                    sorted_blocks[j + 1] = temp;
                }
            }
        }
        
        // 计算外部碎片
        let mut last_end = heap_start;
        for i in 0..block_count {
            let block = &blocks[sorted_blocks[i]];
            if block.addr > last_end {
                let gap_size = block.addr - last_end;
                analysis.external_fragmentation += gap_size;
                analysis.free_block_count += 1;
                analysis.largest_free_block = analysis.largest_free_block.max(gap_size);
            }
            last_end = block.end_addr();
        }
        
        // 最后一个空隙
        if last_end < heap_end {
            let gap_size = heap_end - last_end;
            analysis.external_fragmentation += gap_size;
            analysis.free_block_count += 1;
            analysis.largest_free_block = analysis.largest_free_block.max(gap_size);
        }
        
        // 计算碎片分数
        let total_free = analysis.external_fragmentation;
        if total_free > 0 {
            analysis.fragmentation_score = 
                ((analysis.free_block_count as f32 / (total_free / 4096) as f32) * 100.0) as u8;
        }
        
        analysis
    }
    
    /// 碎片分析结果
    #[derive(Debug)]
    pub struct FragmentationAnalysis {
        pub external_fragmentation: usize,
        pub internal_fragmentation: usize,
        pub largest_free_block: usize,
        pub free_block_count: usize,
        pub fragmentation_score: u8,
    }
}

/// 获取时间戳（简化实现）
fn get_timestamp() -> u64 {
    static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
    COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
}