use std::sync::{Arc, Mutex};

// ANCHOR: db_struct
#[salsa::db]
#[derive(Default, Clone)]
pub struct CalcDatabaseImpl {
    storage: salsa::Storage<Self>,

    // The logs are only used for testing and demonstrating reuse:
    logs: Arc<Mutex<Option<Vec<String>>>>,
}
// ANCHOR_END: db_struct

impl CalcDatabaseImpl {
    /// Enable logging of each salsa event.
    #[cfg(test)]
    pub fn enable_logging(&self) {
        let mut logs = self.logs.lock().unwrap();
        if logs.is_none() {
            *logs = Some(vec![]);
        }
    }

    #[cfg(test)]
    pub fn take_logs(&self) -> Vec<String> {
        let mut logs = self.logs.lock().unwrap();
        if let Some(logs) = &mut *logs {
            std::mem::take(logs)
        } else {
            vec![]
        }
    }
}

// ANCHOR: db_impl
#[salsa::db]
impl salsa::Database for CalcDatabaseImpl {
    fn salsa_event(&self, event: &dyn Fn() -> salsa::Event) {
        let event = event();
        eprintln!("Event: {event:?}");
        // Log interesting events, if logging is enabled
        if let Some(logs) = &mut *self.logs.lock().unwrap() {
            // only log interesting events
            if let salsa::EventKind::WillExecute { .. } = event.kind {
                logs.push(format!("Event: {event:?}"));
            }
        }
    }
}
// ANCHOR_END: db_impl
