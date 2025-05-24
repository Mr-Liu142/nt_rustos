// nt_rustos/src/trap/ds/context.rs

//! # Trap and Task Context Structures
//!
//! Defines the data structures for saving and restoring processor state during
//! traps and context switches.

use super::types::TrapCause;
use core::fmt;

/// # Trap Context
///
/// This struct precisely matches the register layout saved by `trap_entry.asm`.
/// It holds the complete state of a hart at the moment a trap occurs.
/// The order and size of fields are critical and must not be altered without
/// updating the corresponding assembly code.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapContext {
    /// General-purpose registers x0-x31.
    pub x: [usize; 32],
    /// Supervisor Status Register (`sstatus`).
    pub sstatus: usize,
    /// Supervisor Exception Program Counter (`sepc`).
    pub sepc: usize,
    /// Supervisor Cause Register (`scause`).
    pub scause: usize,
    /// Supervisor Trap Value Register (`stval`).
    pub stval: usize,
}

impl TrapContext {
    /// Creates a new, zero-initialized `TrapContext`.
    pub const fn new() -> Self {
        Self {
            x: [0; 32],
            sstatus: 0,
            sepc: 0,
            scause: 0,
            stval: 0,
        }
    }

    /// Interprets the `scause` register to get the high-level trap cause.
    pub fn cause(&self) -> TrapCause {
        TrapCause::from_bits(self.scause)
    }

    /// Modifies the `sepc` to set the return address after the trap is handled.
    /// For most exceptions, this needs to be advanced to the next instruction.
    pub fn advance_sepc(&mut self) {
        // A standard RISC-V instruction is 4 bytes long.
        // Compressed instructions are 2 bytes. For simplicity and robustness in
        // a general handler, we advance by 4. Specific handlers (e.g., breakpoint)
        // might need more nuanced logic if they need to analyze the instruction.
        self.sepc += 4;
    }

    /// Sets the return value of a function call (e.g., for syscalls).
    /// The `a0` register (x[10]) is conventionally used for return values.
    pub fn set_return_value(&mut self, value: usize) {
        self.x[10] = value;
    }
}

/// # Task Context
///
/// This struct holds the minimal state required for a cooperative context switch
/// between tasks (or kernel threads). It only saves callee-saved registers,
/// as the caller-saved registers are expected to be managed by the compiler
/// across function calls.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TaskContext {
    /// Return Address (`ra`). This determines where the task will resume execution.
    ra: usize,
    /// Stack Pointer (`sp`).
    sp: usize,
    /// Callee-saved registers s0-s11.
    s: [usize; 12],
}

impl TaskContext {
    /// Creates a new, zero-initialized `TaskContext`.
    pub const fn new() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }

    /// Creates a new `TaskContext` prepared to start execution at a given entry point and stack.
    ///
    /// # Arguments
    /// * `entry_point` - The address of the function where the task should begin.
    /// * `stack_top` - The top address of the stack allocated for this task.
    pub fn new_for_task(entry_point: usize, stack_top: usize) -> Self {
        Self {
            ra: entry_point,
            sp: stack_top,
            s: [0; 12], // Callee-saved registers are initially zero.
        }
    }
}