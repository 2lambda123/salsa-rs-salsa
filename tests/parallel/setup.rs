use crate::signal::Signal;
use salsa::Database;
use salsa::ParallelDatabase;
use salsa::Snapshot;
use std::cell::Cell;
use std::sync::Arc;

salsa::query_group! {
    pub(crate) trait ParDatabase: Knobs + salsa::ParallelDatabase {
        fn input(key: char) -> usize {
            type Input;
            storage input;
        }

        fn sum(key: &'static str) -> usize {
            type Sum;
        }

        fn sum2(key: &'static str) -> usize {
            type Sum2;
        }

        fn sum3(key: &'static str) -> usize {
            type Sum3;
        }

        fn snapshot_me() -> () {
            type SnapshotMe;
        }
    }
}

/// Various "knobs" and utilities used by tests to force
/// a certain behavior.
pub(crate) trait Knobs {
    fn knobs(&self) -> &KnobsStruct;

    fn signal(&self, stage: usize);

    fn wait_for(&self, stage: usize);
}

pub(crate) trait WithValue<T> {
    fn with_value<R>(&self, value: T, closure: impl FnOnce() -> R) -> R;
}

impl<T> WithValue<T> for Cell<T> {
    fn with_value<R>(&self, value: T, closure: impl FnOnce() -> R) -> R {
        let old_value = self.replace(value);

        let result = closure();

        self.set(old_value);

        result
    }
}

/// Various "knobs" that can be used to customize how the queries
/// behave on one specific thread. Note that this state is
/// intentionally thread-local (apart from `signal`).
#[derive(Clone, Default)]
pub(crate) struct KnobsStruct {
    /// A kind of flexible barrier used to coordinate execution across
    /// threads to ensure we reach various weird states.
    pub(crate) signal: Arc<Signal>,

    /// When this database is about to block, send a signal.
    pub(crate) signal_on_will_block: Cell<usize>,

    /// Invocations of `sum` will signal this stage on entry.
    pub(crate) sum_signal_on_entry: Cell<usize>,

    /// Invocations of `sum` will wait for this stage on entry.
    pub(crate) sum_wait_for_on_entry: Cell<usize>,

    /// If true, invocations of `sum` will panic before they exit.
    pub(crate) sum_should_panic: Cell<bool>,

    /// If true, invocations of `sum` will wait for cancellation before
    /// they exit.
    pub(crate) sum_wait_for_cancellation: Cell<bool>,

    /// Invocations of `sum` will wait for this stage prior to exiting.
    pub(crate) sum_wait_for_on_exit: Cell<usize>,

    /// Invocations of `sum` will signal this stage prior to exiting.
    pub(crate) sum_signal_on_exit: Cell<usize>,
}

fn sum(db: &impl ParDatabase, key: &'static str) -> usize {
    let mut sum = 0;

    db.signal(db.knobs().sum_signal_on_entry.get());

    db.wait_for(db.knobs().sum_wait_for_on_entry.get());

    if db.knobs().sum_should_panic.get() {
        panic!("query set to panic before exit")
    }

    for ch in key.chars() {
        sum += db.input(ch);
    }

    if db.knobs().sum_wait_for_cancellation.get() {
        log::debug!("waiting for cancellation");
        while !db.salsa_runtime().is_current_revision_canceled() {
            std::thread::yield_now();
        }
        log::debug!("cancellation observed");
        return std::usize::MAX; // when we are cancelled, we return usize::MAX.
    }

    db.wait_for(db.knobs().sum_wait_for_on_exit.get());

    db.signal(db.knobs().sum_signal_on_exit.get());

    sum
}

fn sum2(db: &impl ParDatabase, key: &'static str) -> usize {
    db.sum(key)
}

fn sum3(db: &impl ParDatabase, key: &'static str) -> usize {
    db.sum2(key)
}

fn snapshot_me(db: &impl ParDatabase) {
    // this should panic
    db.snapshot();
}

#[derive(Default)]
pub struct ParDatabaseImpl {
    runtime: salsa::Runtime<ParDatabaseImpl>,
    knobs: KnobsStruct,
}

impl Database for ParDatabaseImpl {
    fn salsa_runtime(&self) -> &salsa::Runtime<ParDatabaseImpl> {
        &self.runtime
    }

    fn salsa_event(&self, event_fn: impl Fn() -> salsa::Event<Self>) {
        let event = event_fn();
        match event.kind {
            salsa::EventKind::WillBlockOn { .. } => {
                self.signal(self.knobs().signal_on_will_block.get());
            }

            _ => {}
        }
    }
}

impl ParallelDatabase for ParDatabaseImpl {
    fn snapshot(&self) -> Snapshot<Self> {
        Snapshot::new(ParDatabaseImpl {
            runtime: self.runtime.snapshot(self),
            knobs: self.knobs.clone(),
        })
    }
}

impl Knobs for ParDatabaseImpl {
    fn knobs(&self) -> &KnobsStruct {
        &self.knobs
    }

    fn signal(&self, stage: usize) {
        self.knobs.signal.signal(stage);
    }

    fn wait_for(&self, stage: usize) {
        self.knobs.signal.wait_for(stage);
    }
}

salsa::database_storage! {
    pub struct DatabaseImplStorage for ParDatabaseImpl {
        impl ParDatabase {
            fn input() for Input;
            fn sum() for Sum;
            fn sum2() for Sum2;
            fn sum3() for Sum3;
            fn snapshot_me() for SnapshotMe;
        }
    }
}
