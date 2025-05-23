// 控制台功能测试模块

use super::{TestCase, TestResult, TestRunner};
use crate::{console, println, debug_print};

/// 测试基本字符输出
fn test_basic_char_output() -> TestResult {
    console::print_char('A');
    console::print_char('B');
    console::print_char('C');
    console::print_char('\n');
    TestResult::Pass
}

/// 测试字符串输出
fn test_string_output() -> TestResult {
    console::print_str("Hello from string output test\n");
    TestResult::Pass
}

/// 测试数字输出 - 十进制
fn test_decimal_output() -> TestResult {
    console::print_str("Decimal numbers: ");
    console::print_num(0);
    console::print_str(", ");
    console::print_num(42);
    console::print_str(", ");
    console::print_num(12345);
    console::print_str("\n");
    TestResult::Pass
}

/// 测试数字输出 - 十六进制
fn test_hex_output() -> TestResult {
    console::print_str("Hex numbers: ");
    console::print_hex(0);
    console::print_str(", ");
    console::print_hex(255);
    console::print_str(", ");
    console::print_hex(0xDEADBEEF);
    console::print_str("\n");
    TestResult::Pass
}

/// 测试数字输出 - 八进制
fn test_octal_output() -> TestResult {
    console::print_str("Octal numbers: ");
    console::print_oct(0);
    console::print_str(", ");
    console::print_oct(64);
    console::print_str(", ");
    console::print_oct(0o777);
    console::print_str("\n");
    TestResult::Pass
}

/// 测试格式化输出宏
fn test_format_macros() -> TestResult {
    println!("Testing println macro: {}", "success");
    println!("Numbers: {} {} {}", 42, 0xFF, 0o77);
    TestResult::Pass
}

/// 测试彩色输出宏
fn test_color_output() -> TestResult {
    use crate::{info_print, warn_print, error_print};
    
    info_print!("This is an info message from test");
    warn_print!("This is a warning message from test");
    error_print!("This is an error message from test");
    TestResult::Pass
}

/// 测试调试输出宏
fn test_debug_output() -> TestResult {
    debug_print!("Debug message with line info");
    TestResult::Pass
}

/// 控制台测试用例列表
const CONSOLE_TESTS: &[TestCase] = &[
    TestCase {
        name: "basic_char_output",
        func: test_basic_char_output,
        description: "Test basic character output functionality"
    },
    TestCase {
        name: "string_output", 
        func: test_string_output,
        description: "Test string output functionality"
    },
    TestCase {
        name: "decimal_output",
        func: test_decimal_output,
        description: "Test decimal number output"
    },
    TestCase {
        name: "hex_output",
        func: test_hex_output,
        description: "Test hexadecimal number output"
    },
    TestCase {
        name: "octal_output",
        func: test_octal_output,
        description: "Test octal number output"
    },
    TestCase {
        name: "format_macros",
        func: test_format_macros,
        description: "Test formatting macros (println!)"
    },
    TestCase {
        name: "color_output",
        func: test_color_output,
        description: "Test colored output macros"
    },
    TestCase {
        name: "debug_output",
        func: test_debug_output,
        description: "Test debug output with file/line info"
    },
];

/// 运行所有控制台测试
pub fn run_console_tests(runner: &mut TestRunner) {
    runner.run_suite("Console", CONSOLE_TESTS);
}