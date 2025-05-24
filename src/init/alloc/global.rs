// 全局分配器实现
// 实现GlobalAlloc trait，为Rust标准库提供内存分配接口

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{self, NonNull};
use super::allocator::{ThreadSafeEarlyAllocator, AllocError};
use super::handover::{AllocPurpose, HandoverInfo};
use crate::{error_print, warn_print, debug_print};

/// 全局早期分配器实例
pub static GLOBAL_EARLY_ALLOCATOR: EarlyGlobalAllocator = EarlyGlobalAllocator::new();

/// 全局分配器结构体
#[derive(Clone, Copy)]
pub struct EarlyGlobalAllocator {
    // 空结构体，实际的分配器通过全局静态变量访问
}

// 全局分配器实例 - 内部使用
static ALLOCATOR_INSTANCE: ThreadSafeEarlyAllocator = ThreadSafeEarlyAllocator::new();

impl EarlyGlobalAllocator {
    /// 创建新的全局分配器
    pub const fn new() -> Self {
        Self {}
    }
    
    /// 初始化全局分配器
    pub fn init(&self, heap_start: usize, heap_size: usize) -> Result<(), AllocError> {
        ALLOCATOR_INSTANCE.init(heap_start, heap_size)
    }
    
    /// 设置分配用途（简化实现）
    pub fn set_purpose(&self, _ptr: *mut u8, _purpose: AllocPurpose) -> Result<(), AllocError> {
        Ok(())
    }
    
    /// 获取统计信息
    pub fn stats(&self) -> Option<super::metadata::AllocStats> {
        ALLOCATOR_INSTANCE.stats()
    }
    
    /// 准备接管
    pub fn prepare_handover(&self) -> Option<advanced::EarlyBox<HandoverInfo>> {
        ALLOCATOR_INSTANCE.prepare_handover()
    }
    
    /// 冻结分配器
    pub fn freeze(&self) -> Result<(), AllocError> {
        ALLOCATOR_INSTANCE.freeze()
    }
    
    /// 执行完整性检查
    pub fn integrity_check(&self) -> Result<(), AllocError> {
        ALLOCATOR_INSTANCE.integrity_check()
    }
    
    /// 安全的分配接口（带错误返回）
    pub fn safe_alloc(&self, layout: Layout) -> Result<NonNull<u8>, AllocError> {
        // 验证布局参数
        if layout.size() == 0 {
            return Err(AllocError::InvalidParameter);
        }
        
        if !layout.align().is_power_of_two() || layout.align() > 4096 {
            return Err(AllocError::InvalidAlignment);
        }
        
        // 检查大小是否合理（防止整数溢出）
        if layout.size() > isize::MAX as usize - layout.align() {
            return Err(AllocError::InvalidParameter);
        }
        
        match ALLOCATOR_INSTANCE.alloc_aligned(layout.size(), layout.align()) {
            Some(ptr) => Ok(ptr),
            None => Err(AllocError::OutOfMemory),
        }
    }
    
    /// 分配内存（原始接口）
    pub fn alloc_raw(&self, size: usize) -> Option<NonNull<u8>> {
        ALLOCATOR_INSTANCE.alloc(size)
    }
    
    /// 对齐分配内存（原始接口）
    pub fn alloc_aligned_raw(&self, size: usize, align: usize) -> Option<NonNull<u8>> {
        ALLOCATOR_INSTANCE.alloc_aligned(size, align)
    }
    
    /// 释放内存（原始接口）
    pub fn dealloc_raw(&self, ptr: NonNull<u8>) -> Result<(), AllocError> {
        ALLOCATOR_INSTANCE.dealloc(ptr)
    }
    
    /// 安全的释放接口
    pub fn safe_dealloc(&self, ptr: *mut u8, _layout: Layout) -> Result<(), AllocError> {
        if ptr.is_null() {
            return Err(AllocError::NullPointer);
        }
        
        if let Some(non_null_ptr) = NonNull::new(ptr) {
            ALLOCATOR_INSTANCE.dealloc(non_null_ptr)
        } else {
            Err(AllocError::NullPointer)
        }
    }
    
    /// 分配并清零
    pub fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe {
            let ptr = self.alloc(layout);
            if !ptr.is_null() {
                ptr::write_bytes(ptr, 0, layout.size());
            }
            ptr
        }
    }
    
    /// 重新分配（简单实现）
    pub fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ptr.is_null() {
            return unsafe { 
                self.alloc(Layout::from_size_align(new_size, layout.align()).unwrap_or(layout))
            };
        }
        
        if new_size == 0 {
            unsafe {
                self.dealloc(ptr, layout);
            }
            return ptr::null_mut();
        }
        
        // 分配新的内存
        let new_layout = match Layout::from_size_align(new_size, layout.align()) {
            Ok(l) => l,
            Err(_) => return ptr::null_mut(),
        };
        
        let new_ptr = unsafe { self.alloc(new_layout) };
        if new_ptr.is_null() {
            return ptr::null_mut();
        }
        
        // 复制数据
        unsafe {
            let copy_size = layout.size().min(new_size);
            ptr::copy_nonoverlapping(ptr, new_ptr, copy_size);
        }
        
        // 释放旧内存
        unsafe {
            self.dealloc(ptr, layout);
        }
        
        new_ptr
    }
}

unsafe impl GlobalAlloc for EarlyGlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match ALLOCATOR_INSTANCE.alloc_aligned(layout.size(), layout.align()) {
            Some(ptr) => ptr.as_ptr(),
            None => {
                error_print!("Global allocation failed: size={}, align={}", 
                           layout.size(), layout.align());
                ptr::null_mut()
            }
        }
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            warn_print!("Attempt to deallocate null pointer");
            return;
        }
        
        if let Some(non_null_ptr) = NonNull::new(ptr) {
            if let Err(e) = ALLOCATOR_INSTANCE.dealloc(non_null_ptr) {
                error_print!("Global deallocation failed: {:?}, ptr=0x{:x}, size={}", 
                           e, ptr as usize, layout.size());
            }
        }
    }
    
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = self.alloc(layout);
        if !ptr.is_null() {
            ptr::write_bytes(ptr, 0, layout.size());
        }
        ptr
    }
    
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ptr.is_null() {
            return self.alloc(Layout::from_size_align(new_size, layout.align()).unwrap_or(layout));
        }
        
        if new_size == 0 {
            self.dealloc(ptr, layout);
            return ptr::null_mut();
        }
        
        // 分配新的内存
        let new_layout = match Layout::from_size_align(new_size, layout.align()) {
            Ok(l) => l,
            Err(_) => return ptr::null_mut(),
        };
        
        let new_ptr = self.alloc(new_layout);
        if new_ptr.is_null() {
            return ptr::null_mut();
        }
        
        // 复制数据
        let copy_size = layout.size().min(new_size);
        ptr::copy_nonoverlapping(ptr, new_ptr, copy_size);
        
        // 释放旧内存
        self.dealloc(ptr, layout);
        
        new_ptr
    }
}

/// 内存分配错误处理函数
#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    error_print!("Memory allocation error!");
    error_print!("Requested: size={} bytes, align={}", layout.size(), layout.align());
    
    // 尝试打印分配器状态
    if let Some(stats) = ALLOCATOR_INSTANCE.stats() {
        error_print!("Allocator stats:");
        error_print!("  Total: {} KB", stats.total_size / 1024);
        error_print!("  Used: {} KB ({}%)", stats.used_size / 1024, stats.usage_percent());
        error_print!("  Free: {} KB", stats.free_size / 1024);
        error_print!("  Allocations: {}", stats.total_allocs);
        error_print!("  Fragmentation: {}%", stats.fragmentation_estimate());
    }
    
    panic!("Out of memory");
}

/// 高级分配接口（简化版本）
pub mod advanced {
    use super::*;
    use core::mem;
    
    /// 分配特定类型的内存
    pub fn alloc_type<T>() -> Option<NonNull<T>> {
        let layout = Layout::new::<T>();
        GLOBAL_EARLY_ALLOCATOR.safe_alloc(layout)
            .ok()
            .map(|ptr| ptr.cast::<T>())
    }
    
    /// 分配并初始化特定类型的内存
    pub fn alloc_init<T>(value: T) -> Option<NonNull<T>> {
        if let Some(ptr) = alloc_type::<T>() {
            unsafe {
                ptr::write(ptr.as_ptr(), value);
            }
            Some(ptr)
        } else {
            None
        }
    }
    
    /// 智能指针分配器（简化版本）
    // [修改] 为 EarlyBox 添加 #[derive(Debug)]
    #[derive(Debug)]
    pub struct EarlyBox<T> {
        ptr: NonNull<T>,
    }
    
    impl<T> EarlyBox<T> {
        /// 在堆上分配值
        pub fn new(value: T) -> Option<Self> {
            alloc_init(value).map(|ptr| Self { ptr })
        }
        
        /// 泄露值，返回原始指针
        pub fn leak(self) -> NonNull<T> {
            let ptr = self.ptr;
            mem::forget(self);
            ptr
        }
        
        /// 获取引用
        pub fn as_ref(&self) -> &T {
            unsafe { self.ptr.as_ref() }
        }
        
        /// 获取可变引用
        pub fn as_mut(&mut self) -> &mut T {
            unsafe { self.ptr.as_mut() }
        }
        
        /// 设置分配用途
        pub fn set_purpose(&self, purpose: AllocPurpose) -> Result<(), AllocError> {
            GLOBAL_EARLY_ALLOCATOR.set_purpose(self.ptr.as_ptr() as *mut u8, purpose)
        }
    }
    
    impl<T> Drop for EarlyBox<T> {
        fn drop(&mut self) {
            unsafe {
                // 先调用析构函数
                ptr::drop_in_place(self.ptr.as_ptr());
                // 然后释放内存（简化实现：实际上不释放）
            }
        }
    }
    
    impl<T> core::ops::Deref for EarlyBox<T> {
        type Target = T;
        
        fn deref(&self) -> &Self::Target {
            self.as_ref()
        }
    }
    
    impl<T> core::ops::DerefMut for EarlyBox<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.as_mut()
        }
    }
    
    /// 简单的Vec实现（使用标准库的Vec）
    pub type EarlyVec<T> = alloc::vec::Vec<T>;
}

/// 便捷宏
#[macro_export]
macro_rules! early_vec {
    () => {
        alloc::vec::Vec::new()
    };
    ($($x:expr),+ $(,)?) => {
        {
            let mut vec = alloc::vec::Vec::new();
            $(
                vec.push($x);
            )+
            vec
        }
    };
}

#[macro_export]
macro_rules! early_box {
    ($value:expr) => {
        $crate::init::alloc::global::advanced::EarlyBox::new($value)
            .expect("Failed to allocate early box")
    };
}