// 测试模块入口

pub mod console_test;
pub mod sbi_test;

use crate::{println, info_print, warn_print, error_print};

/// 测试结果枚举
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TestResult {
    Pass,
    Fail,
    Skip,
}

/// 测试用例结构体
pub struct TestCase {
    pub name: &'static str,
    pub func: fn() -> TestResult,
    pub description: &'static str,
}

/// 测试运行器
pub struct TestRunner {
    total: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
}

impl TestRunner {
    /// 创建新的测试运行器
    pub fn new() -> Self {
        Self {
            total: 0,
            passed: 0,
            failed: 0,
            skipped: 0,
        }
    }

    /// 运行单个测试用例
    pub fn run_test(&mut self, test: &TestCase) {
        self.total += 1;
        
        println!("Running test: {} - {}", test.name, test.description);
        
        let result = (test.func)();
        
        match result {
            TestResult::Pass => {
                self.passed += 1;
                info_print!("  [PASS] {}", test.name);
            }
            TestResult::Fail => {
                self.failed += 1;
                error_print!("  [FAIL] {}", test.name);
            }
            TestResult::Skip => {
                self.skipped += 1;
                warn_print!("  [SKIP] {}", test.name);
            }
        }
    }

    /// 运行测试套件
    pub fn run_suite(&mut self, suite_name: &str, tests: &[TestCase]) {
        println!("=== {} Test Suite ===", suite_name);
        
        for test in tests {
            self.run_test(test);
        }
        
        println!("=== {} Test Suite Complete ===", suite_name);
    }

    /// 打印测试总结
    pub fn print_summary(&self) {
        println!("=== Test Summary ===");
        println!("Total tests: {}", self.total);
        info_print!("Passed: {}", self.passed);
        if self.failed > 0 {
            error_print!("Failed: {}", self.failed);
        } else {
            info_print!("Failed: {}", self.failed);
        }
        if self.skipped > 0 {
            warn_print!("Skipped: {}", self.skipped);
        } else {
            info_print!("Skipped: {}", self.skipped);
        }
        
        let success_rate = if self.total > 0 {
            (self.passed * 100) / self.total
        } else {
            100
        };
        
        if success_rate == 100 && self.failed == 0 {
            info_print!("Success rate: {}% - All tests passed!", success_rate);
        } else {
            warn_print!("Success rate: {}%", success_rate);
        }
        println!("==================");
    }

    /// 获取是否所有测试都通过
    pub fn all_passed(&self) -> bool {
        self.failed == 0 && self.total > 0
    }
}

/// 运行所有测试
pub fn run_all_tests() {
    let mut runner = TestRunner::new();
    
    // 运行控制台测试
    console_test::run_console_tests(&mut runner);
    
    // 运行SBI测试
    sbi_test::run_sbi_tests(&mut runner);
    
    // 打印最终总结
    runner.print_summary();
    
    if runner.all_passed() {
        info_print!("All test suites completed successfully!");
    } else {
        warn_print!("Some tests failed or were skipped");
    }
}