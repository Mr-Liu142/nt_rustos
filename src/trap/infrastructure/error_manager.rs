// nt_rustos/src/trap/infrastructure/error_manager.rs

//! # Heap-based Error Manager Implementation
//!
//! An implementation of the `ErrorManager` trait that uses heap-allocated
//! collections for storing error handlers and a generic `RingBuffer` for logging.

use crate::trap::collections::RingBuffer;
use crate::trap::ds::{
    self, SystemError, ErrorResult, ErrorSource, ErrorLevel, ErrorLogEntry,
};
use crate::trap::infrastructure::di::traits::ErrorManager;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use spin::Mutex;
use core::sync::atomic::{AtomicBool, Ordering};

const ERROR_LOG_CAPACITY: usize = 256;

type ErrorHandlerFn = fn(&SystemError) -> ErrorResult;

struct ErrorHandlerEntry {
    priority: u8,
    source: Option<ErrorSource>,
    level: Option<ErrorLevel>,
    handler: ErrorHandlerFn,
}

pub struct HeapErrorManager {
    // Handlers are stored in a BTreeMap, keyed by priority, to ensure sorted execution.
    handlers: Mutex<BTreeMap<u8, Vec<ErrorHandlerEntry>>>,
    log: Mutex<RingBuffer<ErrorLogEntry>>,
    panic_mode: AtomicBool,
}

impl HeapErrorManager {
    pub fn new() -> Self {
        Self {
            handlers: Mutex::new(BTreeMap::new()),
            log: Mutex::new(RingBuffer::with_capacity(ERROR_LOG_CAPACITY)),
            panic_mode: AtomicBool::new(false),
        }
    }

    /// Checks if a given handler entry matches a system error.
    fn matches(entry: &ErrorHandlerEntry, error: &SystemError) -> bool {
        if let Some(src) = entry.source {
            if src != error.code.source() {
                return false;
            }
        }
        if let Some(lvl) = entry.level {
            if lvl != error.code.level() {
                return false;
            }
        }
        true
    }
}

impl ErrorManager for HeapErrorManager {
    fn register_handler(
        &mut self,
        priority: u8,
        source: Option<ErrorSource>,
        level: Option<ErrorLevel>,
        handler: ErrorHandlerFn,
    ) -> Result<(), ()> {
        let entry = ErrorHandlerEntry {
            priority,
            source,
            level,
            handler,
        };

        let mut handlers = self.handlers.lock();
        handlers.entry(priority).or_insert_with(Vec::new).push(entry);
        Ok(())
    }

    fn handle_error(&self, error: SystemError) -> ErrorResult {
        if self.is_panic_mode() && !error.code.is_fatal() {
            // In panic mode, only process new fatal errors. Log others and ignore.
            self.log_error(error, ErrorResult::Unhandled);
            return ErrorResult::Unhandled;
        }
        
        if error.code.is_fatal() {
            self.enter_panic_mode();
        }

        let handlers = self.handlers.lock();
        let mut final_result = ErrorResult::Unhandled;

        // BTreeMap keys are sorted, so we iterate from highest priority (lowest number).
        for (_, entries) in handlers.iter() {
            for entry in entries {
                if Self::matches(entry, &error) {
                    match (entry.handler)(&error) {
                        ErrorResult::Handled => {
                            // Stop processing, the error is fully handled.
                            self.log_error(error, ErrorResult::Handled);
                            return ErrorResult::Handled;
                        }
                        ErrorResult::Partial => {
                            // Mark as partially handled and continue.
                            final_result = ErrorResult::Partial;
                        }
                        ErrorResult::Unhandled => {
                            // Continue to the next handler.
                        }
                    }
                }
            }
        }
        
        self.log_error(error, final_result);
        final_result
    }

    fn log_error(&self, error: SystemError, result: ErrorResult) {
        let log_entry = ErrorLogEntry { error, result };
        self.log.lock().push(log_entry);
    }
    
    fn is_panic_mode(&self) -> bool {
        self.panic_mode.load(Ordering::Relaxed)
    }

    fn enter_panic_mode(&self) {
        self.panic_mode.store(true, Ordering::SeqCst);
    }
}