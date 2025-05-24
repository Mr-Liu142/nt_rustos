// nt_rustos/src/trap/ds/types.rs

//! # Trap Type Definitions
//!
//! Defines various enums and structs related to RISC-V trap causes and types,
//! adhering to the hardware specification.

use core::fmt;

/// Defines the mode of the trap vector.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(usize)]
pub enum TrapMode {
    /// All traps are handled by a single entry function (`stvec`).
    Direct = 0,
    /// Different trap types can have different handlers, if the hardware supports it.
    Vectored = 1,
}

/// Supervisor-level interrupts available in S-mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(usize)]
pub enum Interrupt {
    SupervisorSoft = 1,
    SupervisorTimer = 5,
    SupervisorExternal = 9,
}

/// Supervisor-level exceptions.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(usize)]
pub enum Exception {
    InstructionMisaligned = 0,
    InstructionFault = 1,
    IllegalInstruction = 2,
    Breakpoint = 3,
    LoadMisaligned = 4,
    LoadFault = 5,
    StoreMisaligned = 6,
    StoreFault = 7,
    UserEnvCall = 8,
    SupervisorEnvCall = 9,
    InstructionPageFault = 12,
    LoadPageFault = 13,
    StorePageFault = 15,
}

/// A comprehensive enum representing all handled trap types.
/// This abstraction simplifies handler registration and dispatch.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TrapType {
    TimerInterrupt,
    ExternalInterrupt,
    SoftwareInterrupt,
    SystemCall,
    InstructionPageFault,
    LoadPageFault,
    StorePageFault,
    InstructionAccessFault,
    LoadAccessFault,
    StoreAccessFault,
    IllegalInstruction,
    Breakpoint,
    InstructionMisaligned,
    LoadMisaligned,
    StoreMisaligned,
    Unknown,
}

impl TrapType {
    /// The total number of distinct trap types defined.
    pub const COUNT: usize = 16;

    /// Converts an index into a `TrapType`. Useful for iterating over all types.
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(TrapType::TimerInterrupt),
            1 => Some(TrapType::ExternalInterrupt),
            2 => Some(TrapType::SoftwareInterrupt),
            3 => Some(TrapType::SystemCall),
            4 => Some(TrapType::InstructionPageFault),
            5 => Some(TrapType::LoadPageFault),
            6 => Some(TrapType::StorePageFault),
            7 => Some(TrapType::InstructionAccessFault),
            8 => Some(TrapType::LoadAccessFault),
            9 => Some(TrapType::StoreAccessFault),
            10 => Some(TrapType::IllegalInstruction),
            11 => Some(TrapType::Breakpoint),
            12 => Some(TrapType::InstructionMisaligned),
            13 => Some(TrapType::LoadMisaligned),
            14 => Some(TrapType::StoreMisaligned),
            15 => Some(TrapType::Unknown),
            _ => None,
        }
    }
}


/// A wrapper for the `scause` register, providing a safe interface to interpret its value.
#[derive(Copy, Clone)]
pub struct TrapCause {
    bits: usize,
}

impl TrapCause {
    /// Creates a `TrapCause` from the raw bits of the `scause` register.
    pub const fn from_bits(bits: usize) -> Self {
        Self { bits }
    }

    /// Returns the raw bits of the `scause` register.
    pub const fn bits(&self) -> usize {
        self.bits
    }

    /// Checks if the cause is an interrupt (as opposed to an exception).
    /// The most significant bit of `scause` is set for interrupts.
    pub fn is_interrupt(&self) -> bool {
        self.bits >> (core::mem::size_of::<usize>() * 8 - 1) & 1 != 0
    }

    /// Returns the interrupt or exception code.
    pub fn code(&self) -> usize {
        self.bits & !(1 << (core::mem::size_of::<usize>() * 8 - 1))
    }

    /// Converts the raw `scause` bits into the high-level `TrapType` enum.
    pub fn to_trap_type(&self) -> TrapType {
        if self.is_interrupt() {
            match self.code() {
                1 => TrapType::SoftwareInterrupt,
                5 => TrapType::TimerInterrupt,
                9 => TrapType::ExternalInterrupt,
                _ => TrapType::Unknown,
            }
        } else {
            match self.code() {
                0 => TrapType::InstructionMisaligned,
                1 => TrapType::InstructionAccessFault,
                2 => TrapType::IllegalInstruction,
                3 => TrapType::Breakpoint,
                4 => TrapType::LoadMisaligned,
                5 => TrapType::LoadAccessFault,
                6 => TrapType::StoreMisaligned,
                7 => TrapType::StoreAccessFault,
                8 | 9 => TrapType::SystemCall, // Both U-mode and S-mode ecalls map to SystemCall
                12 => TrapType::InstructionPageFault,
                13 => TrapType::LoadPageFault,
                15 => TrapType::StorePageFault,
                _ => TrapType::Unknown,
            }
        }
    }
}

impl fmt::Debug for TrapCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cause_type = if self.is_interrupt() { "Interrupt" } else { "Exception" };
        write!(
            f,
            "TrapCause::{:?}::{:?} (code: {}, raw: {:#x})",
            cause_type,
            self.to_trap_type(),
            self.code(),
            self.bits()
        )
    }
}