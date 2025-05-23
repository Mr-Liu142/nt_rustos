// SBI功能测试模块

use super::{TestCase, TestResult, TestRunner};
use crate::{util::sbi, println};

/// 测试SBI基础扩展
fn test_sbi_base_extension() -> TestResult {
    match sbi::base::get_spec_version() {
        Ok(version) => {
            println!("  SBI Spec Version: 0x{:x}", version);
            TestResult::Pass
        }
        Err(_) => {
            println!("  Failed to get SBI spec version");
            TestResult::Fail
        }
    }
}

/// 测试SBI扩展探测
fn test_sbi_extension_probe() -> TestResult {
    let extensions = [
        ("Timer", sbi::extension_ids::TIMER),
        ("IPI", sbi::extension_ids::IPI),
        ("RFENCE", sbi::extension_ids::RFENCE),
        ("HSM", sbi::extension_ids::HSM),
    ];

    let mut available_count = 0;
    
    for (name, ext_id) in extensions.iter() {
        let is_available = sbi::info::is_extension_available(*ext_id);
        println!("  {} Extension: {}", name, 
                if is_available { "Available" } else { "Not Available" });
        if is_available {
            available_count += 1;
        }
    }

    if available_count >= 1 {
        TestResult::Pass
    } else {
        TestResult::Fail
    }
}

/// 测试定时器扩展
fn test_timer_extension() -> TestResult {
    match sbi::timer::set_timer(1000000u64) {
        Ok(_) => {
            println!("  Timer set successfully");
            TestResult::Pass
        }
        Err(_) => {
            println!("  Failed to set timer");
            TestResult::Fail
        }
    }
}

/// 测试控制台扩展
fn test_console_extension() -> TestResult {
    match sbi::console::putchar('T') {
        Ok(_) => {
            println!("");
            println!("  Console putchar test passed");
            TestResult::Pass
        }
        Err(_) => {
            println!("  Console putchar test failed");
            TestResult::Fail
        }
    }
}

/// SBI测试用例列表
const SBI_TESTS: &[TestCase] = &[
    TestCase {
        name: "sbi_base_extension",
        func: test_sbi_base_extension,
        description: "Test SBI base extension functionality"
    },
    TestCase {
        name: "sbi_extension_probe",
        func: test_sbi_extension_probe,
        description: "Test SBI extension availability probing"
    },
    TestCase {
        name: "timer_extension",
        func: test_timer_extension,
        description: "Test SBI timer extension"
    },
    TestCase {
        name: "console_extension",
        func: test_console_extension,
        description: "Test SBI console functionality"
    },
];

/// 运行所有SBI测试
pub fn run_sbi_tests(runner: &mut TestRunner) {
    runner.run_suite("SBI", SBI_TESTS);
}