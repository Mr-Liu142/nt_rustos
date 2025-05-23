#![no_std]
#![no_main]

use core::arch::asm;
use nt_rustos::{STACK_SIZE, clear_bss, init, main_loop};

// 用于存放栈的内存区域
#[link_section = ".bss.stack"]
static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

/// 程序入口点 - 汇编代码会跳转到这里
#[no_mangle]
#[link_section = ".text.entry"]
fn _start() -> ! {
    unsafe {
        // 设置栈指针到栈顶
        let stack_top = STACK.as_ptr().add(STACK_SIZE);
        asm!(
            "mv sp, {0}",
            in(reg) stack_top,
        );
        
        // 清除BSS段 - 将未初始化的静态变量清零
        clear_bss();
        
        // 跳转到Rust主函数
        rust_main();
    }
}

/// Rust主函数 - 系统的真正入口点
#[no_mangle]
fn rust_main() -> ! {
    // 系统初始化
    init();
    
    // 进入主循环
    main_loop();
}