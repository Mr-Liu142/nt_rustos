// nt_rustos/src/trap/infrastructure/di/mod.rs

//! # Dependency Injection System - Global Access and Initialization
//!
//! Manages the global instance of the `TrapSystem` and provides safe
//! mechanisms for its initialization and access.

use super::container::TrapSystem;
use super::traits::{HandlerManager, ErrorManager, ContextManager, HardwareController};
use crate::trap::ds::{self, TrapContext, TrapMode};
use crate::trap::infrastructure::{
    handler_manager::HeapHandlerManager,
    error_manager::HeapErrorManager,
    context_manager::HeapContextManager,
    low_level, // For LowLevelHardwareController
};
use alloc::boxed::Box;
use alloc::sync::Arc;
use spin::Mutex;
use core::sync::atomic::{AtomicBool, Ordering};

/// The global `TrapSystem` instance, protected by a `Mutex` for safe access.
static GLOBAL_TRAP_SYSTEM: Mutex<Option<TrapSystem>> = Mutex::new(None);

/// Flag to ensure the trap system is initialized only once.
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Concrete implementation for `HardwareController`.
struct LowLevelHardwareController;
impl HardwareController for LowLevelHardwareController {
    fn init_trap_vector(&self, mode: ds::TrapMode) {
        low_level::init_trap_vector(mode);
    }
    fn enable_interrupts(&self) -> bool {
        low_level::enable_interrupts()
    }
    fn disable_interrupts(&self) -> bool {
        low_level::disable_interrupts()
    }
    fn restore_interrupts(&self, was_enabled: bool) {
        low_level::restore_interrupts(was_enabled);
    }
}

/// Initializes the global trap system.
///
/// This function should be called once during kernel startup. It sets up all
/// necessary managers and the `TrapSystem` container.
///
/// # Arguments
/// * `mode` - The trap mode (Direct or Vectored) for `stvec`.
///
/// # Panics
/// Panics if called more than once.
pub fn initialize_trap_system(mode: TrapMode) {
    if INITIALIZED.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed).is_err() {
        panic!("Trap system already initialized!");
    }

    // Create instances of the concrete managers.
    let handler_manager = Arc::new(HeapHandlerManager::new());
    let error_manager = Arc::new(HeapErrorManager::new());
    let context_manager = Arc::new(HeapContextManager::new()); // Pass handler_manager if needed for cleanup
    let hardware_controller = Box::new(LowLevelHardwareController);

    // Create and initialize the TrapSystem container.
    let trap_system = TrapSystem::new(
        handler_manager, // Arc::clone(&handler_manager) if handler_manager is used elsewhere directly
        error_manager,
        context_manager,
        hardware_controller,
    );
    trap_system.initialize(mode);

    // Register default/enhanced handlers here.
    register_default_enhanced_handlers(trap_system.handler_manager());


    // Store the initialized system globally.
    *GLOBAL_TRAP_SYSTEM.lock() = Some(trap_system);

    // nt_rustos::println!("Trap system initialized with mode: {:?}", mode); // Assuming println exists
}

/// Provides safe, read-only access to the global `TrapSystem`.
///
/// # Arguments
/// * `f` - A closure that takes an immutable reference to the `TrapSystem`.
///
/// # Panics
/// Panics if the trap system has not been initialized.
pub fn with_trap_system<F, R>(f: F) -> R
where
    F: FnOnce(&TrapSystem) -> R,
{
    let guard = GLOBAL_TRAP_SYSTEM.lock();
    let ts = guard.as_ref().expect("Trap system not initialized yet. Call initialize_trap_system first.");
    f(ts)
}

/// This is the C-callable function invoked by `low_level::handle_trap`.
/// It bridges the gap from the assembly context to the Rust `TrapSystem`.
pub(super) fn dispatch_trap(context_ptr: *mut TrapContext) {
    let context = unsafe {
        // Safety: context_ptr is assumed to be valid, coming directly from the
        // CPU's stack pointer after context save in assembly.
        &mut *context_ptr
    };

    with_trap_system(|ts| {
        ts.handle_trap(context);
    });
}

/// Checks if the trap system has been initialized.
pub fn is_initialized() -> bool {
    INITIALIZED.load(Ordering::Relaxed)
}


// Helper function to register default and enhanced handlers
// This would typically call functions from an "enhanced_handlers" module similar to the original.
// For brevity, we'll define stubs or simple handlers here.
fn register_default_enhanced_handlers(handler_manager: Arc<dyn HandlerManager>) {
    // Example: Register a handler for Page Faults
    fn page_fault_handler(ctx: &mut ds::TrapContext) -> ds::TrapHandlerResult {
        // In a real system, this would call the ErrorManager or panic with details
        // For now, just print and make it unhandled to trigger the default container logic
        // nt_rustos::println!(
        //     "Default Page Fault Handler: SEPC={:#x}, STVAL={:#x}, SCAUSE={:?}",
        //     ctx.sepc,
        //     ctx.stval,
        //     ctx.cause()
        // );
        // This should ideally create a SystemError and pass it to the error manager.
        // For production, ensure this path leads to a controlled panic or recovery.
        ds::TrapHandlerResult::Pass // Let the TrapSystem container log it as unhandled.
    }
    
    fn illegal_instruction_handler(ctx: &mut ds::TrapContext) -> ds::TrapHandlerResult {
        ds::TrapHandlerResult::Pass
    }

    let page_fault_entry = Arc::new(RwLock::new(ds::HandlerEntry {
        handler: page_fault_handler,
        priority: 10, // High priority for critical faults
        description: "Default Page Fault Handler",
        protection_level: ds::ProtectionLevel::Kernel,
        registrar_id: ds::KERNEL_REGISTRAR_ID,
        context_id: None,
    }));
    handler_manager.register(ds::TrapType::LoadPageFault, Arc::clone(&page_fault_entry)).expect("Failed to register LPF handler");
    handler_manager.register(ds::TrapType::StorePageFault, Arc::clone(&page_fault_entry)).expect("Failed to register SPF handler");
    handler_manager.register(ds::TrapType::InstructionPageFault, Arc::clone(&page_fault_entry)).expect("Failed to register IPF handler");

    let illegal_inst_entry = Arc::new(RwLock::new(ds::HandlerEntry {
        handler: illegal_instruction_handler,
        priority: 10,
        description: "Default Illegal Instruction Handler",
        protection_level: ds::ProtectionLevel::Kernel,
        registrar_id: ds::KERNEL_REGISTRAR_ID,
        context_id: None,
    }));
    handler_manager.register(ds::TrapType::IllegalInstruction, illegal_inst_entry).expect("Failed to register II handler");
    
    // Register other critical default handlers (Breakpoint, Misaligned, AccessFault, Unknown)
    // similarly, potentially calling out to more detailed "enhanced_handler" functions.
}