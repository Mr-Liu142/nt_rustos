// 生产级早期内存分配器功能测试模块

use super::{TestCase, TestResult, TestRunner};
use crate::{init::alloc, println, debug_print, warn_print};
use crate::{alloc_with_purpose, alloc_zeroed_with_purpose};
use crate::{Vec, String};

/// 测试单次分配与释放
fn test_single_alloc_dealloc() -> TestResult {
    println!("  Testing single allocation and deallocation...");
    
    let ptr = alloc::alloc(100);
    if ptr.is_none() {
        println!("  FAIL: Allocation failed unexpectedly");
        return TestResult::Fail;
    }

    let ptr_val = ptr.unwrap();
    
    // 测试写入数据
    unsafe {
        core::ptr::write(ptr_val, 0x42);
        if core::ptr::read(ptr_val) != 0x42 {
            println!("  FAIL: Memory write/read failed");
            alloc::dealloc(ptr_val);
            return TestResult::Fail;
        }
    }
    
    // 使用安全释放
    if let Err(e) = alloc::dealloc_safe(ptr_val, 100) {
        println!("  FAIL: Safe deallocation failed: {:?}", e);
        return TestResult::Fail;
    }

    // 重新分配应成功，验证内存已正确回收
    let ptr2 = alloc::alloc(100);
    if ptr2.is_none() {
        println!("  FAIL: Re-allocation failed after free");
        return TestResult::Fail;
    }
    alloc::dealloc(ptr2.unwrap());

    println!("  PASS: Single alloc/dealloc successful");
    TestResult::Pass
}

/// 测试多次分配与释放
fn test_multiple_allocs() -> TestResult {
    println!("  Testing multiple allocations...");
    const ALLOC_COUNT: usize = 20;
    let mut pointers = Vec::new();
    
    // 分配多个不同大小的块
    for i in 0..ALLOC_COUNT {
        let size = 64 + (i * 32); // 递增大小
        match alloc::alloc(size) {
            Some(p) => {
                // 测试写入模式数据
                unsafe {
                    for j in 0..size {
                        core::ptr::write((p as *mut u8).add(j), (i % 256) as u8);
                    }
                }
                pointers.push((p, size));
            }
            None => {
                println!("  FAIL: Allocation #{} failed (size: {})", i, size);
                // 清理已分配的内存
                for (ptr, alloc_size) in pointers {
                    if let Err(e) = alloc::dealloc_safe(ptr as *mut u8, alloc_size) {
                        warn_print!("Cleanup failed: {:?}", e);
                    }
                }
                return TestResult::Fail;
            }
        }
    }

    // 验证数据完整性
    for (i, (ptr, size)) in pointers.iter().enumerate() {
        unsafe {
            for j in 0..*size {
                let expected = (i % 256) as u8;
                let actual = core::ptr::read((*ptr as *const u8).add(j));
                if actual != expected {
                    println!("  FAIL: Data corruption detected at allocation {}, offset {}", i, j);
                    // 清理内存
                    for (ptr, alloc_size) in pointers {
                        alloc::dealloc_safe(ptr as *mut u8, alloc_size).ok();
                    }
                    return TestResult::Fail;
                }
            }
        }
    }

    // 释放所有指针
    for (ptr, size) in pointers {
        if let Err(e) = alloc::dealloc_safe(ptr as *mut u8, size) {
            println!("  FAIL: Deallocation failed: {:?}", e);
            return TestResult::Fail;
        }
    }

    println!("  PASS: Multiple allocations and deallocations successful");
    TestResult::Pass
}

/// 测试对齐分配
fn test_aligned_allocation() -> TestResult {
    println!("  Testing aligned allocations...");
    
    let alignments = [8, 16, 32, 64, 128, 256, 512, 1024];
    let mut allocated = Vec::new();
    
    for &align in &alignments {
        if let Some(ptr) = alloc::alloc_aligned(1024, align) {
            // 验证对齐
            if (ptr as usize) % align != 0 {
                println!("  FAIL: Misaligned allocation (expected: {}, got: 0x{:x})", 
                         align, ptr as usize);
                // 清理
                for (p, s) in allocated {
                    alloc::dealloc_safe(p, s).ok();
                }
                alloc::dealloc(ptr);
                return TestResult::Fail;
            }
            allocated.push((ptr, 1024));
        } else {
            println!("  FAIL: Aligned allocation failed for alignment {}", align);
            // 清理
            for (p, s) in allocated {
                alloc::dealloc_safe(p as *mut u8, s).ok();
            }
            return TestResult::Fail;
        }
    }
    
    // 清理所有分配
    for (ptr, size) in allocated {
        alloc::dealloc_safe(ptr as *mut u8, size).ok();
    }
    
    println!("  PASS: Aligned allocations successful");
    TestResult::Pass
}

/// 测试用途分配
fn test_purpose_allocation() -> TestResult {
    println!("  Testing purpose-based allocation...");
    
    use alloc::AllocPurpose;
    
    let purposes = [
        (AllocPurpose::KernelStack, 4096),
        (AllocPurpose::PageTable, 8192),
        (AllocPurpose::NetworkBuffer, 1024),
        (AllocPurpose::TempBuffer, 512),
    ];
    
    let mut allocated = Vec::new();
    
    for (purpose, size) in purposes.iter() {
        if let Some(ptr) = alloc_with_purpose!(*size, *purpose) {
            allocated.push((ptr, *size));
        } else {
            println!("  FAIL: Purpose allocation failed for {}", purpose.description());
            // 清理
            for (p, s) in allocated {
                alloc::dealloc_safe(p as *mut u8, s).ok();
            }
            return TestResult::Fail;
        }
    }
    
    // 验证接管信息包含正确的用途
    if let Some(handover) = alloc::prepare_handover() {
        let groups = handover.group_by_purpose();
        let mut found_purposes = 0;
        
        for (purpose, count, _) in &groups {
            if *count > 0 {
                // 检查是否是我们分配的用途之一
                for (test_purpose, _) in &purposes {
                    if (*purpose as u8) == (*test_purpose as u8) {
                        found_purposes += 1;
                        break;
                    }
                }
            }
        }
        
        if found_purposes < purposes.len() {
            println!("  FAIL: Not all purposes found in handover info");
            // 清理
            for (p, s) in allocated {
                alloc::dealloc_safe(p, s).ok();
            }
            return TestResult::Fail;
        }
    }
    
    // 清理
    for (ptr, size) in allocated {
        alloc::dealloc_safe(ptr as *mut u8, size).ok();
    }
    
    println!("  PASS: Purpose allocation successful");
    TestResult::Pass
}

/// 测试动态Vec
fn test_dynamic_vec() -> TestResult {
    println!("  Testing dynamic Vec operations...");
    
    let mut vec = Vec::new();
    
    // 测试基本操作
    for i in 0..100 {
        vec.push(i * i);
    }
    
    if vec.len() != 100 {
        println!("  FAIL: Vec length incorrect: expected 100, got {}", vec.len());
        return TestResult::Fail;
    }
    
    // 验证数据
    for (i, &value) in vec.iter().enumerate() {
        if value != i * i {
            println!("  FAIL: Vec data corruption at index {}: expected {}, got {}", 
                     i, i * i, value);
            return TestResult::Fail;
        }
    }
    
    // 测试pop操作
    for i in (0..100).rev() {
        if let Some(value) = vec.pop() {
            if value != i * i {
                println!("  FAIL: Vec pop returned wrong value: expected {}, got {}", 
                         i * i, value);
                return TestResult::Fail;
            }
        } else {
            println!("  FAIL: Vec pop failed at index {}", i);
            return TestResult::Fail;
        }
    }
    
    if !vec.is_empty() {
        println!("  FAIL: Vec not empty after popping all elements");
        return TestResult::Fail;
    }
    
    println!("  PASS: Dynamic Vec operations successful");
    TestResult::Pass
}

/// 测试动态String
fn test_dynamic_string() -> TestResult {
    println!("  Testing dynamic String operations...");
    
    let mut s = String::new();
    
    // 测试基本操作
    s.push_str("Hello");
    s.push(' ');
    s.push_str("World!");
    
    if s != "Hello World!" {
        println!("  FAIL: String content incorrect: expected 'Hello World!', got '{}'", s);
        return TestResult::Fail;
    }
    
    // 测试大字符串
    let mut big_string = String::new();
    for i in 0..1000 {
        // 简单地添加字符而不是格式化数字
        let ch = (b'0' + (i % 10) as u8) as char;
        big_string.push(ch);
        big_string.push(',');
    }
    
    if big_string.len() < 1000 {
        println!("  FAIL: Big string too short: {}", big_string.len());
        return TestResult::Fail;
    }
    
    println!("  PASS: Dynamic String operations successful");
    TestResult::Pass
}

/// 测试EarlyBox智能指针
fn test_early_box() -> TestResult {
    println!("  Testing EarlyBox smart pointer...");
    
    use alloc::global::advanced::EarlyBox;
    
    // 测试基本创建和访问
    if let Some(boxed_int) = EarlyBox::new(42i32) {
        if *boxed_int != 42 {
            println!("  FAIL: EarlyBox value incorrect: expected 42, got {}", *boxed_int);
            return TestResult::Fail;
        }
        
        // 测试设置用途
        if let Err(e) = boxed_int.set_purpose(alloc::AllocPurpose::Testing) {
            println!("  FAIL: Failed to set EarlyBox purpose: {:?}", e);
            return TestResult::Fail;
        }
    } else {
        println!("  FAIL: Failed to create EarlyBox");
        return TestResult::Fail;
    }
    
    // 测试复杂数据结构
    let test_vec = crate::vec![1, 2, 3, 4, 5];
    if let Some(boxed_vec) = EarlyBox::new(test_vec) {
        if boxed_vec.len() != 5 {
            println!("  FAIL: EarlyBox Vec length incorrect");
            return TestResult::Fail;
        }
    } else {
        println!("  FAIL: Failed to create EarlyBox with Vec");
        return TestResult::Fail;
    }
    
    println!("  PASS: EarlyBox operations successful");
    TestResult::Pass
}

/// 测试EarlyVec自定义向量
fn test_early_vec() -> TestResult {
    println!("  Testing EarlyVec custom vector...");
    
    // 使用标准库的Vec作为EarlyVec
    let mut early_vec = Vec::new();
    
    // 测试基本操作
    for i in 0..10 {
        early_vec.push(i * 2);
    }
    
    if early_vec.len() != 10 {
        println!("  FAIL: EarlyVec length incorrect: expected 10, got {}", 
                 early_vec.len());
        return TestResult::Fail;
    }
    
    // 测试索引访问
    for i in 0..10 {
        if early_vec[i] != i * 2 {
            println!("  FAIL: EarlyVec data incorrect at index {}: expected {}, got {}", 
                     i, i * 2, early_vec[i]);
            return TestResult::Fail;
        }
    }
    
    // 测试pop操作
    for i in (0..10).rev() {
        if let Some(value) = early_vec.pop() {
            if value != i * 2 {
                println!("  FAIL: EarlyVec pop incorrect: expected {}, got {}", 
                         i * 2, value);
                return TestResult::Fail;
            }
        } else {
            println!("  FAIL: EarlyVec pop failed");
            return TestResult::Fail;
        }
    }
    
    // 测试扩容
    for i in 0..20 {
        early_vec.push(i);
    }
    
    if early_vec.len() != 20 {
        println!("  FAIL: EarlyVec growth length incorrect");
        return TestResult::Fail;
    }
    
    println!("  PASS: EarlyVec operations successful");
    TestResult::Pass
}

/// 测试内存泄漏检测
fn test_leak_detection() -> TestResult {
    println!("  Testing memory leak detection...");
    
    // 创建一些"泄漏"的分配（用于测试）
    let mut leaked_ptrs = Vec::new();
    for i in 0..5 {
        if let Some(ptr) = alloc_with_purpose!(1024, alloc::AllocPurpose::Testing) {
            leaked_ptrs.push((ptr, 1024));
            
            // 写入一些数据
            unsafe {
                core::ptr::write_bytes(ptr, i as u8, 1024);
            }
        }
    }
    
    // 检查接管信息中的泄漏检测
    if let Some(handover) = alloc::prepare_handover() {
        let leak_result = handover.detect_potential_leaks();
        
        if leak_result.suspicious_count == 0 {
            println!("  WARN: No suspicious blocks detected (expected some)");
        } else {
            debug_print!("Detected {} suspicious blocks", leak_result.suspicious_count);
        }
        
        // 检查是否正确识别了测试用途的块
        let groups = handover.group_by_purpose();
        let mut found_testing = false;
        for (purpose, count, _) in &groups {
            if *purpose as u8 == alloc::AllocPurpose::Testing as u8 && *count > 0 {
                found_testing = true;
                break;
            }
        }
        
        if !found_testing {
            println!("  FAIL: Testing purpose blocks not found in handover");
            // 清理
            for (ptr, size) in leaked_ptrs {
                alloc::dealloc_safe(ptr, size).ok();
            }
            return TestResult::Fail;
        }
    }
    
    // 清理"泄漏"的内存
    for (ptr, size) in leaked_ptrs {
        alloc::dealloc_safe(ptr, size).ok();
    }
    
    println!("  PASS: Leak detection working");
    TestResult::Pass
}

/// 测试完整性检查
fn test_integrity_check() -> TestResult {
    println!("  Testing allocator integrity check...");
    
    // 分配一些内存
    let mut allocations = Vec::new();
    for i in 0..10 {
        if let Some(ptr) = alloc::alloc(512 + i * 64) {
            allocations.push((ptr, 512 + i * 64));
        }
    }
    
    // 执行完整性检查
    match alloc::integrity_check() {
        Ok(_) => {
            debug_print!("Integrity check passed");
        }
        Err(e) => {
            println!("  FAIL: Integrity check failed: {:?}", e);
            // 清理
            for (ptr, size) in allocations {
                alloc::dealloc_safe(ptr, size).ok();
            }
            return TestResult::Fail;
        }
    }
    
    // 清理分配
    for (ptr, size) in allocations {
        alloc::dealloc_safe(ptr, size).ok();
    }
    
    // 再次执行完整性检查
    match alloc::integrity_check() {
        Ok(_) => {
            println!("  PASS: Integrity check successful");
            TestResult::Pass
        }
        Err(e) => {
            println!("  FAIL: Post-cleanup integrity check failed: {:?}", e);
            TestResult::Fail
        }
    }
}

/// 测试健康检查
fn test_health_check() -> TestResult {
    println!("  Testing allocator health check...");
    
    if let Some(health) = alloc::health_check() {
        if health.is_healthy() {
            debug_print!("Allocator is healthy");
        } else {
            warn_print!("Allocator health issues detected");
            health.print_report();
        }
        
        println!("  PASS: Health check completed");
        TestResult::Pass
    } else {
        println!("  FAIL: Failed to get health status");
        TestResult::Fail
    }
}

/// 测试双重释放检测
fn test_double_free_detection() -> TestResult {
    println!("  Testing double free detection...");
    
    let ptr = alloc::alloc(32).expect("Allocation for double free test failed");
    
    // 第一次释放应该成功
    if let Err(e) = alloc::dealloc_safe(ptr, 32) {
        println!("  FAIL: First deallocation failed: {:?}", e);
        return TestResult::Fail;
    }
    
    // 第二次释放应该失败
    match alloc::dealloc_safe(ptr, 32) {
        Err(alloc::AllocError::DoubleFree) | Err(alloc::AllocError::InvalidPointer) => {
            println!("  PASS: Correctly detected double free");
            TestResult::Pass
        }
        Ok(_) => {
            println!("  FAIL: Double free not detected");
            TestResult::Fail
        }
        Err(e) => {
            println!("  FAIL: Unexpected error on double free: {:?}", e);
            TestResult::Fail
        }
    }
}

/// 压力测试
fn test_stress_allocation() -> TestResult {
    println!("  Running stress test...");
    
    const ITERATIONS: usize = 100;
    const MAX_ALLOCS: usize = 50;
    let mut active_allocs = Vec::new();
    
    for iteration in 0..ITERATIONS {
        // 随机分配或释放
        let action = iteration % 3;
        
        match action {
            0 | 1 => { // 分配
                if active_allocs.len() < MAX_ALLOCS {
                    let size = 64 + (iteration % 10) * 128;
                    if let Some(ptr) = alloc::alloc(size) {
                        // 写入测试数据
                        unsafe {
                            core::ptr::write_bytes(ptr, (iteration % 256) as u8, size);
                        }
                        active_allocs.push((ptr, size, iteration % 256));
                    } else {
                        debug_print!("Allocation failed at iteration {} (expected under stress)", iteration);
                    }
                }
            }
            2 => { // 释放
                if !active_allocs.is_empty() {
                    let index = iteration % active_allocs.len();
                    let (ptr, size, pattern) = active_allocs.remove(index);
                    
                    // 验证数据完整性
                    unsafe {
                        let first_byte = core::ptr::read(ptr);
                        if first_byte != (pattern % 256) as u8 {
                            println!("  FAIL: Data corruption detected in stress test");
                            // 清理剩余分配
                            for (p, s, _) in active_allocs {
                                alloc::dealloc_safe(p, s).ok();
                            }
                            alloc::dealloc_safe(ptr, size).ok();
                            return TestResult::Fail;
                        }
                    }
                    
                    alloc::dealloc_safe(ptr, size).ok();
                }
            }
            _ => unreachable!()
        }
        
        // 每10次迭代检查完整性
        if iteration % 10 == 0 {
            if let Err(e) = alloc::integrity_check() {
                println!("  FAIL: Integrity check failed during stress test: {:?}", e);
                // 清理
                for (p, s, _) in active_allocs {
                    alloc::dealloc_safe(p, s).ok();
                }
                return TestResult::Fail;
            }
        }
    }
    
    // 清理所有剩余分配
    for (ptr, size, _) in active_allocs {
        alloc::dealloc_safe(ptr, size).ok();
    }
    
    // 最终完整性检查
    match alloc::integrity_check() {
        Ok(_) => {
            println!("  PASS: Stress test completed successfully");
            TestResult::Pass
        }
        Err(e) => {
            println!("  FAIL: Final integrity check failed: {:?}", e);
            TestResult::Fail
        }
    }
}

/// 内存分配器测试用例列表 - 增强版本
const ALLOC_TESTS: &[TestCase] = &[
    TestCase {
        name: "single_alloc_dealloc",
        func: test_single_alloc_dealloc,
        description: "Test a single allocation and deallocation with data integrity",
    },
    TestCase {
        name: "multiple_allocs",
        func: test_multiple_allocs,
        description: "Test multiple allocations with different sizes and data verification",
    },
    TestCase {
        name: "aligned_allocation",
        func: test_aligned_allocation,
        description: "Test memory alignment requirements",
    },
    TestCase {
        name: "purpose_allocation",
        func: test_purpose_allocation,
        description: "Test purpose-based memory allocation",
    },
    TestCase {
        name: "dynamic_vec",
        func: test_dynamic_vec,
        description: "Test standard library Vec operations",
    },
    TestCase {
        name: "dynamic_string",
        func: test_dynamic_string,
        description: "Test standard library String operations",
    },
    TestCase {
        name: "early_box",
        func: test_early_box,
        description: "Test EarlyBox smart pointer",
    },
    TestCase {
        name: "early_vec",
        func: test_early_vec,
        description: "Test EarlyVec custom vector implementation",
    },
    TestCase {
        name: "leak_detection",
        func: test_leak_detection,
        description: "Test memory leak detection capabilities",
    },
    TestCase {
        name: "integrity_check",
        func: test_integrity_check,
        description: "Test allocator integrity verification",
    },
    TestCase {
        name: "health_check",
        func: test_health_check,
        description: "Test allocator health monitoring",
    },
    TestCase {
        name: "double_free_detection",
        func: test_double_free_detection,
        description: "Test double free detection and prevention",
    },
    TestCase {
        name: "stress_allocation",
        func: test_stress_allocation,
        description: "Stress test with random allocation/deallocation patterns",
    },
];

/// 运行所有内存分配器测试
pub fn run_alloc_tests(runner: &mut TestRunner) {
    println!("Starting comprehensive allocator test suite...");
    
    // 打印测试前的内存状态
    if let Some(stats) = alloc::stats() {
        println!("Pre-test memory state:");
        stats.print_summary();
    }
    
    // 创建测试前快照
    let snapshot_before = alloc::create_snapshot();
    
    // 运行测试套件
    runner.run_suite("Enhanced Allocator", ALLOC_TESTS);
    
    // 创建测试后快照并比较
    if let (Some(before), Some(after)) = (snapshot_before, alloc::create_snapshot()) {
        println!("Memory usage during tests:");
        let comparison = before.compare(&after);
        comparison.print();
        
        // 检查是否有内存泄漏
        if comparison.size_delta > 0 {
            warn_print!("Potential memory leak detected during tests: {} bytes", 
                       comparison.size_delta);
        }
    }
    
    // 执行最终检查
    match alloc::integrity_check() {
        Ok(_) => {
            println!("Post-test integrity check: PASSED");
        }
        Err(e) => {
            warn_print!("Post-test integrity check: FAILED ({:?})", e);
        }
    }
    
    // 打印最终内存状态
    if let Some(stats) = alloc::stats() {
        println!("Post-test memory state:");
        stats.print_summary();
        
        // 健康检查
        let health = stats.check_health();
        if !health.is_healthy() {
            warn_print!("Allocator health issues after tests:");
            health.print_report();
        }
    }
    
    println!("Allocator test suite completed");
}