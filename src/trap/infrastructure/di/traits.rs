// nt_rustos/src/trap/infrastructure/di/traits.rs

//! # Dependency Injection Traits
//!
//! Defines the traits (interfaces) for the core components of the trap subsystem.
//! This allows for a clean separation of concerns and enables dependency injection.

use crate::trap::ds::{
    self,
    TrapContext, TrapType, TrapHandlerResult, SystemError, ErrorResult, HandlerHandle,
    RegistrarId,
};
use alloc::sync::Arc;
use spin::RwLock;

/// Interface for the Trap Handler Manager.
///
/// Responsible for registering, unregistering, and dispatching trap handlers.
pub trait HandlerManager: Send + Sync {
    /// Registers a trap handler.
    fn register(
        &self,
        trap_type: TrapType,
        entry: Arc<RwLock<ds::HandlerEntry>>,
    ) -> Result<HandlerHandle, ()>;

    /// Unregisters a trap handler using its handle.
    fn unregister(&self, handle: HandlerHandle, requester_id: RegistrarId) -> Result<(), ()>;

    /// Transfers ownership of a handler to a new registrar.
    fn transfer_ownership(
        &self,
        handle: HandlerHandle,
        current_owner: RegistrarId,
        new_owner: RegistrarId,
    ) -> Result<(), ()>;

    /// Dispatches a trap to the appropriate registered handlers.
    fn dispatch(&self, context: &mut TrapContext) -> TrapHandlerResult;
    
    /// Unregisters all handlers associated with a given context ID.
    fn unregister_for_context(&self, context_id: u64);
}

/// Interface for the Error Manager.
///
/// Responsible for registering, dispatching, and logging system errors.
pub trait ErrorManager: Send + Sync {
    /// Registers an error handler.
    fn register_handler(
        &mut self,
        priority: u8,
        source: Option<ds::ErrorSource>,
        level: Option<ds::ErrorLevel>,
        handler: fn(&SystemError) -> ErrorResult,
    ) -> Result<(), ()>;

    /// Handles a system error by dispatching it to registered handlers.
    fn handle_error(&self, error: SystemError) -> ErrorResult;

    /// Logs an error to the system error log.
    fn log_error(&self, error: SystemError, result: ErrorResult);
    
    /// Checks if the system is currently in a panic state.
    fn is_panic_mode(&self) -> bool;
    
    /// Enters panic mode.
    fn enter_panic_mode(&self);
}

/// Interface for the Context Manager.
///
/// Responsible for managing the lifecycle of context-aware objects, such as processes,
/// and ensuring their associated resources (like trap handlers) are cleaned up.
pub trait ContextManager: Send + Sync {
    // In a full OS, this trait would have methods like `create_process`, `destroy_process`, etc.
    // For this refactoring, its main role is to integrate with the handler manager for cleanup.
    // For now, it can be a marker trait, with its implementation holding the logic.
}

/// Interface for Hardware Control.
///
/// Provides an abstraction for basic hardware-level trap control operations.
/// In this refactored system, many of these operations are simple passthroughs
/// to the `crate::trap::infrastructure::low_level` module.
pub trait HardwareController: Send + Sync {
    /// Initializes the hardware trap vector.
    fn init_trap_vector(&self, mode: ds::TrapMode);

    /// Enables all supervisor-level interrupts.
    /// Returns `true` if interrupts were previously enabled.
    fn enable_interrupts(&self) -> bool;

    /// Disables all supervisor-level interrupts.
    /// Returns `true` if interrupts were previously enabled.
    fn disable_interrupts(&self) -> bool;

    /// Restores interrupts to a previous state.
    fn restore_interrupts(&self, was_enabled: bool);
}