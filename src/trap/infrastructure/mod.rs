// nt_rustos/src/trap/infrastructure/mod.rs

//! # Trap Infrastructure Module
//!
//! This module provides the core implementation of the trap handling subsystem.
//! It includes the low-level hardware abstractions, the dependency injection (DI)
//! framework, and concrete implementations of the various managers for handlers,
//! errors, and contexts.

// The assembly code for the trap entry point.
pub mod asm;

// The Dependency Injection (DI) framework.
pub mod di;

// Low-level hardware interaction layer.
pub mod low_level;

// Concrete manager implementations.
pub mod error_manager;
pub mod handler_manager;
pub mod context_manager;

// Re-export the main initialization function for the trap system.
pub use di::initialize_trap_system;