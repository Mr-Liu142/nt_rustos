// nt_rustos/src/trap/infrastructure/handler_manager.rs

//! # Heap-based Trap Handler Manager
//!
//! Implements the `HandlerManager` trait using heap-allocated collections for
//! dynamic, priority-aware, and ownership-based handler management.

use crate::trap::ds::{
    self, HandlerEntry, HandlerHandle, RegistrarId, TrapType, TrapHandlerResult
};
use crate::trap::infrastructure::di::traits::HandlerManager;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, RwLock};

type HandlerStore = Arc<RwLock<HandlerEntry>>;
type PriorityMap = BTreeMap<u8, Vec<HandlerStore>>;
type TrapMap = BTreeMap<TrapType, PriorityMap>;

/// A map from a handler's unique ID to its full `HandlerStore` (`Arc<RwLock<...>>`).
/// This allows for O(log N) lookup of any handler by its handle.
type HandleMap = BTreeMap<u64, HandlerStore>;

pub struct HeapHandlerManager {
    /// The primary storage for handlers, organized by trap type and priority.
    handlers: Mutex<TrapMap>,
    /// A secondary map for quick lookups via `HandlerHandle`.
    handle_map: Mutex<HandleMap>,
}

impl HeapHandlerManager {
    pub fn new() -> Self {
        Self {
            handlers: Mutex::new(BTreeMap::new()),
            handle_map: Mutex::new(BTreeMap::new()),
        }
    }
}

impl HandlerManager for HeapHandlerManager {
    fn register(
        &self,
        trap_type: TrapType,
        entry: Arc<RwLock<HandlerEntry>>,
    ) -> Result<HandlerHandle, ()> {
        let handle = {
            let read_entry = entry.read();
            HandlerHandle::generate_id(read_entry.description, trap_type)
        };
        
        let mut handle_map = self.handle_map.lock();
        if handle_map.contains_key(&handle.id()) {
            // A handler with this exact description and type already exists.
            return Err(());
        }
        
        let mut handlers = self.handlers.lock();
        let priority_map = handlers.entry(trap_type).or_insert_with(BTreeMap::new);
        let priority_list = priority_map.entry(entry.read().priority).or_insert_with(Vec::new);
        
        priority_list.push(Arc::clone(&entry));
        handle_map.insert(handle.id(), entry);

        Ok(handle)
    }

    fn unregister(&self, handle: HandlerHandle, requester_id: RegistrarId) -> Result<(), ()> {
        let mut handle_map = self.handle_map.lock();
        let handler_arc = match handle_map.get(&handle.id()) {
            Some(arc) => Arc::clone(arc),
            None => return Err(()), // Handler not found.
        };

        // Check for ownership before proceeding.
        if !handler_arc.read().can_be_unregistered_by(requester_id) {
            return Err(());
        }
        
        // Remove from the primary handler map. This is more complex.
        let mut handlers = self.handlers.lock();
        let (description, trap_type, priority) = {
            let entry = handler_arc.read();
            (entry.description, ds::TrapType::from_index(0).unwrap(), entry.priority) // Placeholder, need to find the correct trap_type
        };
        // This is inefficient. A better way would be to store trap_type in HandlerEntry
        // or have a reverse mapping. For now, we iterate.
        let mut found_trap_type = None;
        for (tt, p_map) in handlers.iter() {
             if let Some(p_vec) = p_map.get(&priority) {
                 if p_vec.iter().any(|h| h.read().description == description) {
                     found_trap_type = Some(*tt);
                     break;
                 }
             }
        }
        
        if let Some(tt) = found_trap_type {
            if let Some(priority_map) = handlers.get_mut(&tt) {
                if let Some(priority_list) = priority_map.get_mut(&priority) {
                    priority_list.retain(|h| h.read().description != description);
                    if priority_list.is_empty() {
                        priority_map.remove(&priority);
                    }
                }
            }
        } else {
             return Err(());
        }

        // Finally, remove from the handle map.
        handle_map.remove(&handle.id());
        
        Ok(())
    }

    fn transfer_ownership(
        &self,
        handle: HandlerHandle,
        current_owner: RegistrarId,
        new_owner: RegistrarId,
    ) -> Result<(), ()> {
        let handle_map = self.handle_map.lock();
        let handler_arc = match handle_map.get(&handle.id()) {
            Some(arc) => arc,
            None => return Err(()),
        };

        let mut entry = handler_arc.write();
        // Kernel can transfer any ownership. Others must be the current owner.
        if entry.registrar_id != current_owner && current_owner != ds::KERNEL_REGISTRAR_ID {
            return Err(());
        }

        entry.registrar_id = new_owner;
        Ok(())
    }

    fn dispatch(&self, context: &mut ds::TrapContext) -> ds::TrapHandlerResult {
        let trap_type = context.cause().to_trap_type();
        let handlers = self.handlers.lock();

        if let Some(priority_map) = handlers.get(&trap_type) {
            // BTreeMap iterates keys (priorities) in ascending order.
            for (_, handlers) in priority_map.iter() {
                for handler_arc in handlers.iter() {
                    let handler_fn = handler_arc.read().handler;
                    match handler_fn(context) {
                        TrapHandlerResult::Handled => return TrapHandlerResult::Handled,
                        TrapHandlerResult::Failed(e) => {
                            // Log the failure and continue to the next handler.
                            // In a real system, this would use the ErrorManager.
                            // println!("Handler failed: {:?}", e);
                            continue;
                        },
                        TrapHandlerResult::Pass => continue,
                    }
                }
            }
        }
        
        // No handler handled the trap.
        TrapHandlerResult::Pass
    }
    
    fn unregister_for_context(&self, context_id: u64) {
        let mut handle_map = self.handle_map.lock();
        let mut handlers = self.handlers.lock();
        
        let mut handles_to_remove = Vec::new();
        
        // First, collect all handles that need to be removed.
        for (handle_id, handler_arc) in handle_map.iter() {
            if let Some(cid) = handler_arc.read().context_id {
                if cid == context_id {
                    handles_to_remove.push(*handle_id);
                }
            }
        }
        
        // Now, remove them.
        for handle_id in handles_to_remove {
             if let Some(handler_arc) = handle_map.remove(&handle_id) {
                  let entry = handler_arc.read();
                   // This is inefficient like above.
                  for (_tt, p_map) in handlers.iter_mut() {
                      if let Some(p_vec) = p_map.get_mut(&entry.priority) {
                           p_vec.retain(|h| h.read().description != entry.description);
                      }
                  }
             }
        }
    }
}