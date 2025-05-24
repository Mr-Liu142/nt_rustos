// nt_rustos/src/trap/ds/error.rs

//! # Error Handling Data Structures
//!
//! Defines the types and data structures for the system-wide error handling framework.
//! This design allows for structured error reporting and dispatching.

use core::fmt;
use alloc::string::String;

/// Defines the severity of an error.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ErrorLevel {
    /// An unrecoverable error requiring a system halt.
    Fatal = 0,
    /// A serious error that may require terminating a process or subsystem.
    Critical = 1,
    /// A standard error that can likely be handled.
    Error = 2,
    /// A potential issue that does not prevent correct operation but should be noted.
    Warning = 3,
    /// An informational message.
    Info = 4,
}

/// Identifies the subsystem where an error originated.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ErrorSource {
    Unknown,
    Generic,
    Trap,
    Memory,
    Process,
    FileSystem,
    Device,
    Network,
    Syscall,
}

/// A structured error code, combining source, level, and a specific code.
/// Format: 32-bit integer
/// - Bits 24-31: `ErrorSource`
/// - Bits 16-23: `ErrorLevel`
/// - Bits 0-15:  Specific error number
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ErrorCode(u32);

impl ErrorCode {
    /// Creates a new `ErrorCode`.
    pub const fn new(source: ErrorSource, level: ErrorLevel, code: u16) -> Self {
        Self(((source as u32) << 24) | ((level as u32) << 16) | (code as u32))
    }

    /// Returns the `ErrorSource` part of the code.
    pub fn source(&self) -> ErrorSource {
        match (self.0 >> 24) as u8 {
            1 => ErrorSource::Generic,
            2 => ErrorSource::Trap,
            3 => ErrorSource::Memory,
            4 => ErrorSource::Process,
            5 => ErrorSource::FileSystem,
            6 => ErrorSource::Device,
            7 => ErrorSource::Network,
            8 => ErrorSource::Syscall,
            _ => ErrorSource::Unknown,
        }
    }

    /// Returns the `ErrorLevel` part of the code.
    pub fn level(&self) -> ErrorLevel {
        match ((self.0 >> 16) & 0xFF) as u8 {
            0 => ErrorLevel::Fatal,
            1 => ErrorLevel::Critical,
            2 => ErrorLevel::Error,
            3 => ErrorLevel::Warning,
            4 => ErrorLevel::Info,
            _ => ErrorLevel::Error, // Default to a safe value.
        }
    }

    /// Returns the specific error number.
    pub fn number(&self) -> u16 {
        (self.0 & 0xFFFF) as u16
    }

    /// Checks if the error is fatal.
    pub fn is_fatal(&self) -> bool {
        self.level() == ErrorLevel::Fatal
    }
}

impl fmt::Debug for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ErrorCode({:?}|{:?}|{})",
            self.source(),
            self.level(),
            self.number()
        )
    }
}

/// Represents a complete system error, with context.
#[derive(Debug, Clone)]
pub struct SystemError {
    /// The structured error code.
    pub code: ErrorCode,
    /// A descriptive message about the error.
    pub message: String,
    /// The address associated with the error (e.g., faulting address), if any.
    pub address: Option<usize>,
    /// The instruction pointer where the error occurred.
    pub instruction_pointer: usize,
    /// A timestamp indicating when the error occurred.
    pub timestamp: u64,
}

impl SystemError {
    /// Creates a new `SystemError`.
    pub fn new(
        code: ErrorCode,
        message: impl Into<String>,
        address: Option<usize>,
        instruction_pointer: usize,
        timestamp: u64,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            address,
            instruction_pointer,
            timestamp,
        }
    }
}

impl fmt::Display for SystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SystemError {:?} at IP={:#x}: {}",
            self.code, self.instruction_pointer, self.message
        )?;
        if let Some(addr) = self.address {
            write!(f, " (address: {:#x})", addr)?;
        }
        Ok(())
    }
}


/// The result of an error handler's execution.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ErrorResult {
    /// The error was fully handled and processing can stop.
    Handled,
    /// The error was partially handled; subsequent handlers should still be tried.
    Partial,
    /// The handler did not handle the error; processing should continue.
    Unhandled,
}

/// An entry in the system error log.
#[derive(Debug, Clone)]
pub struct ErrorLogEntry {
    /// The error that was recorded.
    pub error: SystemError,
    /// The final result of the handling process.
    pub result: ErrorResult,
}