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
    
    /// 设置分配用途
    pub fn set_purpose(&self, ptr: *mut u8, purpose: AllocPurpose) -> Result<(), AllocError> {
        if let Some(non_null_ptr) = NonNull::new(ptr) {
            ALLOCATOR_INSTANCE.set_purpose(non_null_ptr, purpose)
        } else {
            Err(AllocError::NullPointer)
        }
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
        if layout.size() == 0 {
            return Err(AllocError::InvalidParameter);
        }
        
        if !layout.align().is_power_of_two() || layout.align() > 4096 {
            return Err(AllocError::InvalidAlignment);
        }
        
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
        
        let new_layout = match Layout::from_size_align(new_size, layout.align()) {
            Ok(l) => l,
            Err(_) => return ptr::null_mut(),
        };
        
        let new_ptr = unsafe { self.alloc(new_layout) };
        if new_ptr.is_null() {
            return ptr::null_mut();
        }
        
        unsafe {
            let copy_size = layout.size().min(new_size);
            ptr::copy_nonoverlapping(ptr, new_ptr, copy_size);
        }
        
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
            None => ptr::null_mut(),
        }
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
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
        self.realloc(ptr, layout, new_size)
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    error_print!("Memory allocation error!");
    error_print!("Requested: size={} bytes, align={}", layout.size(), layout.align());
    
    if let Some(stats) = ALLOCATOR_INSTANCE.stats() {
        error_print!("Allocator stats:");
        stats.print_detailed();
    }
    
    panic!("Out of memory");
}

pub mod advanced {
    use super::*;
    use core::mem;
    
    pub fn alloc_type<T>() -> Option<NonNull<T>> {
        let layout = Layout::new::<T>();
        GLOBAL_EARLY_ALLOCATOR.safe_alloc(layout)
            .ok()
            .map(|ptr| ptr.cast::<T>())
    }
    
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
    
    #[derive(Debug)]
    pub struct EarlyBox<T: ?Sized> {
        ptr: NonNull<T>,
    }
    
    impl<T> EarlyBox<T> {
        pub fn new(value: T) -> Option<Self> {
            alloc_init(value).map(|ptr| Self { ptr })
        }
    }

    impl<T: ?Sized> EarlyBox<T> {
        pub fn leak(b: Self) -> NonNull<T> {
            let ptr = b.ptr;
            mem::forget(b);
            ptr
        }

        pub fn set_purpose(&self, purpose: AllocPurpose) -> Result<(), AllocError> {
            unsafe {
                 GLOBAL_EARLY_ALLOCATOR.set_purpose(self.ptr.as_ptr() as *mut u8, purpose)
            }
        }
    }
    
    impl<T: ?Sized> Drop for EarlyBox<T> {
        fn drop(&mut self) {
            unsafe {
                let layout = Layout::for_value(self.ptr.as_ref());
                ptr::drop_in_place(self.ptr.as_ptr());
                GLOBAL_EARLY_ALLOCATOR.dealloc(self.ptr.as_ptr() as *mut u8, layout);
            }
        }
    }
    
    impl<T: ?Sized> core::ops::Deref for EarlyBox<T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            unsafe { self.ptr.as_ref() }
        }
    }
    
    impl<T: ?Sized> core::ops::DerefMut for EarlyBox<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { self.ptr.as_mut() }
        }
    }
    
    pub type EarlyVec<T> = alloc::vec::Vec<T>;
}

#[macro_export]
macro_rules! early_vec {
    () => { alloc::vec::Vec::new() };
    ($($x:expr),+ $(,)?) => { { let mut vec = alloc::vec::Vec::new(); $( vec.push($x); )+ vec } };
}

#[macro_export]
macro_rules! early_box {
    ($value:expr) => {
        $crate::init::alloc::global::advanced::EarlyBox::new($value)
            .expect("Failed to allocate early box")
    };
}
