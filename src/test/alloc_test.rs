// nt_rustos/src/test/alloc_test.rs

// 早期内存分配器功能测试模块

use super::{TestCase, TestResult, TestRunner};
use crate::{init::alloc, println};

/// 测试单次分配与释放
fn test_single_alloc_dealloc() -> TestResult {
    println!("  Testing single allocation and deallocation...");
    let ptr = alloc::alloc(100);
    if ptr.is_none() {
        println!("  FAIL: Allocation failed unexpectedly");
        return TestResult::Fail;
    }

    let ptr_val = ptr.unwrap();
    alloc::dealloc(ptr_val);

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
    const ALLOC_COUNT: usize = 10;
    let mut pointers = [core::ptr::null_mut(); ALLOC_COUNT];
    
    for i in 0..ALLOC_COUNT {
        pointers[i] = match alloc::alloc(64) {
            Some(p) => p,
            None => {
                println!("  FAIL: Allocation #{} failed", i);
                // 释放已分配的内存
                for j in 0..i {
                    alloc::dealloc(pointers[j]);
                }
                return TestResult::Fail;
            }
        };
    }

    // 释放所有指针
    for i in 0..ALLOC_COUNT {
        alloc::dealloc(pointers[i]);
    }

    println!("  PASS: Multiple allocations and deallocations successful");
    TestResult::Pass
}

/// 测试内存耗尽场景
fn test_out_of_memory() -> TestResult {
    println!("  Testing out of memory scenario...");
    let stats_before = alloc::stats().unwrap();
    let large_size = stats_before.free_size;

    // 尝试分配所有剩余内存
    let ptr = alloc::alloc(large_size);
    if ptr.is_none() {
        println!("  PASS: Correctly failed to allocate a block that is too large");
        return TestResult::Pass; // 这是预期的行为，因为大小可能超过MAX_BLOCK_SIZE或不满足分配策略
    }

    // 尝试分配一个刚好无法满足的小块
    alloc::dealloc(ptr.unwrap()); // 释放上面的块
    let almost_all = stats_before.total_size; // 使用total_size来尝试耗尽
    let ptr2 = alloc::alloc(almost_all);
    if ptr2.is_some() {
        // 如果成功了，再尝试分配一个小块，此时应失败
        let last_ptr = alloc::alloc(128);
        alloc::dealloc(ptr2.unwrap());
        if last_ptr.is_some() {
             alloc::dealloc(last_ptr.unwrap());
             println!("  FAIL: Allocator did not report out of memory");
             return TestResult::Fail;
        }
    }


    println!("  PASS: Out of memory handled correctly");
    TestResult::Pass
}

/// 测试重复释放（Double Free）
fn test_double_free() -> TestResult {
    println!("  Testing double free...");
    let ptr = alloc::alloc(32).expect("Allocation for double free test failed");

    alloc::dealloc(ptr);

    // 预期：dealloc_safe应返回错误，但系统不应崩溃
    // 这个测试主要是为了观察日志中是否有错误输出
    println!("  INFO: Expect a 'Deallocation failed: DoubleFree' error message below.");
    match alloc::dealloc_safe(ptr) {
        Err(alloc::AllocError::DoubleFree) => {
            println!("  PASS: Correctly detected double free");
            TestResult::Pass
        }
        _ => {
            println!("  FAIL: Did not detect double free");
            TestResult::Fail
        }
    }
}

/// 测试不同大小的分配
fn test_various_sizes() -> TestResult {
    println!("  Testing various allocation sizes...");
    // 修复：将 'let' 修改为 'const'，并遵循大写命名约定
    const SIZES: [usize; 8] = [32, 64, 128, 256, 512, 1024, 2048, 4096];
    let mut pointers = [core::ptr::null_mut(); SIZES.len()];

    for (i, &size) in SIZES.iter().enumerate() {
        pointers[i] = match alloc::alloc(size) {
            Some(p) => p,
            None => {
                println!("  FAIL: Allocation of size {} failed", size);
                // 释放已分配的内存
                for j in 0..i {
                    alloc::dealloc(pointers[j]);
                }
                return TestResult::Fail;
            }
        };
    }

    for &ptr in pointers.iter() {
        alloc::dealloc(ptr);
    }

    println!("  PASS: Various sizes allocated and deallocated successfully");
    TestResult::Pass
}


/// 内存分配器测试用例列表
const ALLOC_TESTS: &[TestCase] = &[
    TestCase {
        name: "single_alloc_dealloc",
        func: test_single_alloc_dealloc,
        description: "Test a single allocation and deallocation",
    },
    TestCase {
        name: "multiple_allocs",
        func: test_multiple_allocs,
        description: "Test a series of allocations and deallocations",
    },
    TestCase {
        name: "various_sizes",
        func: test_various_sizes,
        description: "Test allocations of all supported block sizes",
    },
    TestCase {
        name: "out_of_memory",
        func: test_out_of_memory,
        description: "Test behavior when memory is exhausted",
    },
    TestCase {
        name: "double_free",
        func: test_double_free,
        description: "Test the allocator's handling of double free",
    },
];

/// 运行所有内存分配器测试
pub fn run_alloc_tests(runner: &mut TestRunner) {
    runner.run_suite("Allocator", ALLOC_TESTS);
}