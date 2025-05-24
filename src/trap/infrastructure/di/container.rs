// nt_rustos/src/trap/infrastructure/di/container.rs

//! # Trap System Dependency Injection Container
//!
//! Defines the `TrapSystem` struct, which acts as the central container
//! for all major components (managers) of the trap subsystem.

use super::traits::{HandlerManager, ErrorManager, ContextManager, HardwareController};
use crate::trap::ds::{self, TrapContext, SystemError, ErrorResult};
use alloc::boxed::Box;
use alloc::sync::Arc;

pub struct TrapSystem {
    handler_manager: Arc<dyn HandlerManager>,
    error_manager: Arc<dyn ErrorManager>,
    #[allow(dead_code)] // ContextManager is part of the design, might not be fully used initially
    context_manager: Arc<dyn ContextManager>,
    hardware_controller: Box<dyn HardwareController>,
}

impl TrapSystem {
    /// Creates a new `TrapSystem` by injecting its dependencies.
    pub fn new(
        handler_manager: Arc<dyn HandlerManager>,
        error_manager: Arc<dyn ErrorManager>,
        context_manager: Arc<dyn ContextManager>,
        hardware_controller: Box<dyn HardwareController>,
    ) -> Self {
        Self {
            handler_manager,
            error_manager,
            context_manager,
            hardware_controller,
        }
    }

    /// Initializes the trap system components, including the hardware vector.
    pub fn initialize(&self, mode: ds::TrapMode) {
        self.hardware_controller.init_trap_vector(mode);
        // Further initialization of managers can be done here if needed.
    }

    /// The main trap handling routine called from the low-level assembly bridge.
    /// It dispatches the trap to the `HandlerManager`.
    pub fn handle_trap(&self, context: &mut TrapContext) {
        // Before dispatching, one might want to perform some global pre-processing,
        // like incrementing interrupt nesting counters, if not handled at a lower level.

        let result = self.handler_manager.dispatch(context);

        match result {
            ds::TrapHandlerResult::Handled => {
                // Trap was fully handled.
            }
            ds::TrapHandlerResult::Pass => {
                // No registered handler fully handled this trap.
                // This is where a "default unhandled trap" routine would be invoked.
                // For critical unhandled exceptions, this might involve generating a
                // SystemError and passing it to the ErrorManager, or panicking.
                let cause = context.cause();
                let error = SystemError::new(
                    ds::ErrorCode::new(ds::ErrorSource::Trap, ds::ErrorLevel::Critical, cause.code() as u16),
                    alloc::format!("Unhandled trap: {:?}, SEPC: {:#x}, STVAL: {:#x}", cause.to_trap_type(), context.sepc, context.stval),
                    Some(context.stval),
                    context.sepc,
                    0, // Placeholder for timestamp; a real system would get current time.
                );
                self.error_manager.handle_error(error);
            }
            ds::TrapHandlerResult::Failed(trap_err) => {
                // A handler attempted to process but failed internally.
                let cause = context.cause();
                 let error = SystemError::new(
                    ds::ErrorCode::new(ds::ErrorSource::Trap, ds::ErrorLevel::Error, cause.code() as u16),
                    alloc::format!("Trap handler failed for {:?}: {:?}, SEPC: {:#x}", cause.to_trap_type(), trap_err, context.sepc),
                    Some(context.stval),
                    context.sepc,
                    0, 
                );
                self.error_manager.handle_error(error);
            }
        }
        // Global post-processing after dispatch can occur here.
    }

    /// Provides access to the `HandlerManager`.
    pub fn handler_manager(&self) -> Arc<dyn HandlerManager> {
        Arc::clone(&self.handler_manager)
    }

    /// Provides access to the `ErrorManager`.
    pub fn error_manager(&self) -> Arc<dyn ErrorManager> {
        Arc::clone(&self.error_manager)
    }
    
    /// Provides access to the `HardwareController`.
    pub fn hardware_controller(&self) -> &dyn HardwareController {
        &*self.hardware_controller
    }
}