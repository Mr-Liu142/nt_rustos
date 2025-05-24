// nt_rustos/src/trap/infrastructure/low_level.rs

//! # Low-Level Trap Hardware Control
//!
//! This module provides direct control over the RISC-V trap-related CSRs
//! (Control and Status Registers) and includes the assembly entry point for traps.

use crate::trap::ds::{TrapContext, TrapMode};
use core::arch::{asm, global_asm};

// Include the assembly code that handles saving and restoring the trap context.
global_asm!(include_str!("asm/trap_entry.asm"));

// External symbols defined in `trap_entry.asm`.
extern "C" {
    /// The assembly entry point for all traps. It saves the full context.
    fn __trap_entry();
    /// The assembly exit point for all traps. It restores the full context.
    fn __trap_return();
}

/// Initializes the trap subsystem at the hardware level.
///
/// Sets the Supervisor Trap Vector (`stvec`) register to point to our trap entry point.
///
/// # Arguments
///
/// * `mode` - The desired trap mode (`Direct` or `Vectored`).
pub fn init_trap_vector(mode: TrapMode) {
    let stvec_value = __trap_entry as usize | mode as usize;
    unsafe {
        asm!("csrw stvec, {}", in(reg) stvec_value);
    }
}

/// A "C" callable function that is the target of the `call` instruction in `__trap_entry`.
///
/// This function acts as the bridge from assembly to the Rust-based trap dispatching logic.
/// It retrieves the `TrapContext` pointer from the `a0` register and passes it to the
/// high-level dispatcher provided by the `TrapSystem` container.
///
/// # Safety
///
/// This function must only be called from the `__trap_entry` assembly code. The `context`
/// pointer is guaranteed to be valid within the scope of the trap.
#[no_mangle]
pub extern "C" fn handle_trap(context: *mut TrapContext) {
    // This function now delegates directly to the globally managed trap system.
    // The `TrapSystem` will contain the full logic for dispatching the trap.
    crate::trap::infrastructure::di::dispatch_trap(context);
}

/// Enables supervisor-level interrupts globally for the current hart.
///
/// # Returns
///
/// `true` if interrupts were previously enabled, `false` otherwise.
#[inline]
pub fn enable_interrupts() -> bool {
    let mut sstatus: usize;
    unsafe {
        asm!("csrrci {}, sstatus, 1 << 1", out(reg) sstatus);
    }
    // Check if the SIE bit (bit 1) was set previously.
    (sstatus & (1 << 1)) != 0
}

/// Disables supervisor-level interrupts globally for the current hart.
///
/// # Returns
///
/// `true` if interrupts were previously enabled, `false` otherwise.
#[inline]
pub fn disable_interrupts() -> bool {
    let mut sstatus: usize;
    unsafe {
        asm!("csrrc {}, sstatus, 1 << 1", out(reg) sstatus);
    }
    // Check if the SIE bit (bit 1) was set previously.
    (sstatus & (1 << 1)) != 0
}

/// Restores the global interrupt enable state.
///
/// # Arguments
///
/// * `was_enabled` - The previous state of the interrupt flag, as returned by
///   `enable_interrupts` or `disable_interrupts`.
#[inline]
pub fn restore_interrupts(was_enabled: bool) {
    if was_enabled {
        unsafe {
            asm!("csrs sstatus, {}", in(reg) 1 << 1);
        }
    }
}