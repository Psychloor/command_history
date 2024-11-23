use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};

use parking_lot::RwLock;

use crate::traits::{command::Command, command_history::CommandHistory};

pub struct ConcurrentCommandHistory<C: Command + Send + Sync> {
    undo: RwLock<VecDeque<Arc<C>>>,
    redo: RwLock<VecDeque<Arc<C>>>,
    history_limit: AtomicUsize,
    clear_redo_on_execute: AtomicBool,
}

impl<C> ConcurrentCommandHistory<C>
where
    C: Command + Send + Sync,
{
    #[must_use]
    pub fn new(history_limit: NonZeroUsize, clear_redo_on_execute: bool) -> Arc<Self> {
        let limit = history_limit.get();

        Arc::new(Self {
            undo: RwLock::new(VecDeque::with_capacity(limit)),
            redo: RwLock::new(VecDeque::with_capacity(limit)),
            history_limit: AtomicUsize::new(limit),
            clear_redo_on_execute: AtomicBool::new(clear_redo_on_execute),
        })
    }

    pub fn undo_history(&self) -> Option<Vec<Arc<C>>> {
        let undo_lock = self.undo.read();
        if undo_lock.is_empty() {
            return None;
        }

        Some(undo_lock.iter().cloned().collect())
    }

    pub fn redo_history(&self) -> Option<Vec<Arc<C>>> {
        let redo_lock = self.redo.read();
        if redo_lock.is_empty() {
            return None;
        }

        Some(redo_lock.iter().cloned().collect())
    }

    fn push_undo(
        &self,
        command: Arc<C>,
        undo_lock: &mut parking_lot::RwLockWriteGuard<VecDeque<Arc<C>>>,
    ) {
        let limit = self.history_limit.load(Ordering::Relaxed);
        while undo_lock.len() >= limit {
            undo_lock.pop_back();
        }

        undo_lock.push_front(command);
    }

    fn push_redo(
        &self,
        command: Arc<C>,
        redo_lock: &mut parking_lot::RwLockWriteGuard<VecDeque<Arc<C>>>,
    ) {
        let limit = self.history_limit.load(Ordering::Relaxed);
        while redo_lock.len() >= limit {
            redo_lock.pop_back();
        }

        redo_lock.push_front(command);
    }

    pub fn set_clear_redo_on_execute(&self, clear: bool) {
        self.clear_redo_on_execute.store(clear, Ordering::Relaxed);
    }
}

impl<C> CommandHistory<C> for ConcurrentCommandHistory<C>
where
    C: Command + Send + Sync,
{
    fn execute_command(&self, command: C, ctx: &C::Context) {
        let command = Arc::new(command);
        command.execute(ctx);

        let mut undo = self.undo.write();
        self.push_undo(command, &mut undo);

        if self.clear_redo_on_execute.load(Ordering::Relaxed) {
            self.redo.write().clear();
        }
    }

    fn undo(&self, ctx: &C::Context) {
        let mut undo = self.undo.write();
        if let Some(command) = undo.pop_front() {
            command.undo(ctx);

            let mut redo = self.redo.write();
            self.push_redo(command, &mut redo);
        }
    }

    fn redo(&self, ctx: &C::Context) {
        let mut redo = self.redo.write();
        if let Some(command) = redo.pop_front() {
            command.redo(ctx);

            let mut undo = self.undo.write();
            self.push_undo(command, &mut undo);
        }
    }

    fn set_history_limit(&self, limit: NonZeroUsize) {
        assert!(limit.get() > 0);
        let limit = limit.get();

        self.history_limit.store(limit, Ordering::Release);

        let mut undo = self.undo.write();
        while undo.len() > limit {
            undo.pop_back();
        }

        let mut redo = self.redo.write();
        while redo.len() > limit {
            redo.pop_back();
        }
    }

    fn batch_execute(&self, commands: Vec<C>, ctx: &C::Context) {
        let mut undo = self.undo.write();
        for command in commands {
            let command = Arc::new(command);
            command.execute(ctx);

            self.push_undo(command, &mut undo);
        }

        if self.clear_redo_on_execute.load(Ordering::Relaxed) {
            self.redo.write().clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        borrow::Cow, cmp::min, hint::black_box, num::NonZero, panic::AssertUnwindSafe, thread, time,
    };

    use rand::Rng;

    use crate::shared_context::SharedContext;

    use super::*;

    #[derive(Debug)]
    enum TestOperation {
        Increment(i32),
        Decrement(i32),
    }

    struct TestArcCommand {
        operation: TestOperation,
    }

    struct TestArcContext {
        value: i32,
    }

    impl Command for TestArcCommand {
        type Context = SharedContext<TestArcContext>;
        fn execute(&self, ctx: &Self::Context) {
            let mut ctx = ctx.lock();
            match self.operation {
                TestOperation::Increment(value) => ctx.value += value,
                TestOperation::Decrement(value) => ctx.value -= value,
            }
        }

        fn undo(&self, ctx: &Self::Context) {
            let mut ctx = ctx.lock();
            match self.operation {
                TestOperation::Increment(value) => ctx.value -= value,
                TestOperation::Decrement(value) => ctx.value += value,
            }
        }

        fn description(&self) -> Cow<'_, str> {
            match self.operation {
                TestOperation::Increment(value) => {
                    Cow::Owned(format!("TestArcCommand: Increment({value})"))
                }
                TestOperation::Decrement(value) => {
                    Cow::Owned(format!("TestArcCommand: Decrement({value})"))
                }
            }
        }
    }

    #[test]
    fn test_arc_command() {
        let history = ConcurrentCommandHistory::new(NonZeroUsize::new(5).unwrap(), true);
        let ctx = SharedContext::new(TestArcContext { value: 0 });
        let command = TestArcCommand {
            operation: TestOperation::Increment(1),
        };

        history.execute_command(command, &ctx);
        assert_eq!(ctx.lock().value, 1);

        history.undo(&ctx);
        assert_eq!(ctx.lock().value, 0);

        history.redo(&ctx);
        assert_eq!(ctx.lock().value, 1);
    }

    #[test]
    fn test_arc_max_size() {
        let history = ConcurrentCommandHistory::new(NonZero::new(2).unwrap(), true);
        let ctx = SharedContext::new(TestArcContext { value: 0 });
        let command1 = TestArcCommand {
            operation: TestOperation::Increment(1),
        };
        let command2 = TestArcCommand {
            operation: TestOperation::Increment(2),
        };
        let command3 = TestArcCommand {
            operation: TestOperation::Increment(3),
        };

        history.execute_command(command1, &ctx);
        assert_eq!(ctx.lock().value, 1);
        assert_eq!(history.undo.read().len(), 1);
        assert!(history.redo.read().is_empty());

        history.execute_command(command2, &ctx);
        assert_eq!(ctx.lock().value, 3);
        assert_eq!(history.undo.read().len(), 2);
        assert!(history.redo.read().is_empty());

        history.execute_command(command3, &ctx);

        assert_eq!(ctx.lock().value, 6);
        assert_eq!(history.undo.read().len(), 2);
        assert!(history.redo.read().is_empty());
    }

    #[test]
    fn test_arc_batch() {
        let size = rand::thread_rng().gen_range(10..40);
        let history = ConcurrentCommandHistory::new(NonZeroUsize::new(35).unwrap(), true);
        let ctx = SharedContext::new(TestArcContext { value: 0 });

        let mut expected_sum = 0;
        let commands: Vec<_> = (0..size)
            .map(|i| {
                let value = rand::thread_rng().gen_range(1..10);

                let increment = i % 2 == 0;
                if increment {
                    expected_sum += value;
                    TestArcCommand {
                        operation: TestOperation::Increment(value),
                    }
                } else {
                    expected_sum -= value;
                    TestArcCommand {
                        operation: TestOperation::Decrement(value),
                    }
                }
            })
            .collect();

        history.batch_execute(commands, &ctx);

        assert_eq!(ctx.lock().value, expected_sum);
        assert_eq!(history.undo.read().len(), min(size, 35));
    }

    #[test]
    fn test_arc_set_history_limit() {
        let history = ConcurrentCommandHistory::new(NonZero::new(2).unwrap(), true);
        let ctx = SharedContext::new(TestArcContext { value: 0 });
        let command1 = TestArcCommand {
            operation: TestOperation::Increment(1),
        };
        let command2 = TestArcCommand {
            operation: TestOperation::Increment(2),
        };
        let command3 = TestArcCommand {
            operation: TestOperation::Increment(3),
        };

        history.execute_command(command1, &ctx);
        assert_eq!(ctx.lock().value, 1);
        assert_eq!(history.undo.read().len(), 1);
        assert!(history.redo.read().is_empty());

        history.execute_command(command2, &ctx);
        assert_eq!(ctx.lock().value, 3);
        assert_eq!(history.undo.read().len(), 2);
        assert!(history.redo.read().is_empty());

        history.execute_command(command3, &ctx);
        assert_eq!(ctx.lock().value, 6);
        assert_eq!(history.undo.read().len(), 2);
        assert!(history.redo.read().is_empty());

        history.set_history_limit(NonZero::new(1).unwrap());
        assert_eq!(history.undo.read().len(), 1);
        assert!(history.redo.read().is_empty());
    }

    #[test]
    fn test_arc_concurrent_rng() {
        let size = rand::thread_rng().gen_range(5..40);
        let history = ConcurrentCommandHistory::new(NonZero::new(size).unwrap(), true);
        let ctx = SharedContext::new(TestArcContext { value: 0 });

        let mut handles = Vec::new();
        let count = rand::thread_rng().gen_range(10..80);

        for _ in 0..count {
            let value = rand::thread_rng().gen_range(0..20);
            let command = TestArcCommand {
                operation: TestOperation::Increment(value),
            };
            let history_clone = history.clone();
            let ctx_clone = ctx.clone();

            handles.push(black_box(thread::spawn(move || {
                thread::sleep(time::Duration::from_millis(
                    rand::thread_rng().gen_range(0..250),
                ));
                history_clone.execute_command(command, &ctx_clone);
                value
            })));
        }

        let mut sum = 0;
        for handle in handles {
            let result = handle.join();
            assert!(result.is_ok(), "Thread panicked during execution");
            sum += result.expect("Thread should return a value");
        }

        assert_eq!(ctx.lock().value, sum);
        assert_eq!(history.undo.read().len(), min(size, count));
        assert!(history.redo.read().is_empty());

        let mut handles = Vec::new();
        for _ in 0..size {
            let history_clone = Arc::clone(&history);
            let ctx_clone = ctx.clone();

            handles.push(black_box(thread::spawn(move || {
                thread::sleep(time::Duration::from_millis(
                    rand::thread_rng().gen_range(0..200),
                ));
                history_clone.undo(&ctx_clone);
                thread::sleep(time::Duration::from_millis(
                    rand::thread_rng().gen_range(0..200),
                ));
                history_clone.redo(&ctx_clone);
            })));
        }

        for handle in handles {
            assert!(handle.join().is_ok(), "Thread panicked during execution");
        }

        assert_eq!(ctx.lock().value, sum);
        assert_eq!(
            history.undo.read().len(),
            min(size, count),
            "Undo stack size after execution"
        );
        assert!(
            history.redo.read().is_empty(),
            "Redo stack should be empty after execution"
        );
    }

    #[test]
    fn test_command_clears_redo() {
        let history = ConcurrentCommandHistory::new(NonZero::new(5).unwrap(), true);
        let ctx = SharedContext::new(TestArcContext { value: 0 });
        let command = TestArcCommand {
            operation: TestOperation::Increment(1),
        };
        let command2 = TestArcCommand {
            operation: TestOperation::Increment(1),
        };

        history.execute_command(command, &ctx);
        assert_eq!(ctx.lock().value, 1);
        assert_eq!(history.undo.read().len(), 1);
        assert!(history.redo.read().is_empty());

        history.undo(&ctx);
        assert_eq!(ctx.lock().value, 0);
        assert!(history.undo.read().is_empty());
        assert_eq!(history.redo.read().len(), 1);

        history.execute_command(command2, &ctx);
        assert_eq!(ctx.lock().value, 1);
        assert_eq!(history.undo.read().len(), 1);
        assert!(history.redo.read().is_empty());
    }

    #[test]
    fn test_command_dont_clears_redo() {
        let history = ConcurrentCommandHistory::new(NonZero::new(5).unwrap(), false);
        let ctx = SharedContext::new(TestArcContext { value: 0 });
        let command = TestArcCommand {
            operation: TestOperation::Increment(1),
        };
        let command2 = TestArcCommand {
            operation: TestOperation::Increment(1),
        };

        history.execute_command(command, &ctx);
        assert_eq!(ctx.lock().value, 1);
        assert_eq!(history.undo.read().len(), 1);
        assert!(history.redo.read().is_empty());

        history.undo(&ctx);
        assert_eq!(ctx.lock().value, 0);
        assert!(history.undo.read().is_empty());
        assert_eq!(history.redo.read().len(), 1);

        history.execute_command(command2, &ctx);
        assert_eq!(ctx.lock().value, 1);
        assert_eq!(history.undo.read().len(), 1);
        assert_eq!(history.redo.read().len(), 1);
    }

    #[test]
    fn test_panic_safety() {
        let history = ConcurrentCommandHistory::new(NonZero::new(85).unwrap(), true);
        let ctx = SharedContext::new(TestArcContext { value: 0 });
        let command = TestArcCommand {
            operation: TestOperation::Increment(1),
        };

        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            history.execute_command(command, &ctx);
            panic!("Simulated panic");
        }));

        assert!(result.is_err()); // Ensure panic was caught
        assert_eq!(ctx.lock().value, 1); // Ensure state is consistent
    }

    #[test]
    fn test_undo_history() {
        let history = ConcurrentCommandHistory::new(NonZeroUsize::new(5).unwrap(), true);
        let ctx = SharedContext::new(TestArcContext { value: 0 });
        let command1 = TestArcCommand {
            operation: TestOperation::Increment(1),
        };
        let command2 = TestArcCommand {
            operation: TestOperation::Increment(2),
        };

        history.execute_command(command1, &ctx);
        history.execute_command(command2, &ctx);

        let undo_history = history.undo_history().unwrap();
        assert_eq!(undo_history.len(), 2);
        assert_eq!(
            undo_history[0].description(),
            "TestArcCommand: Increment(2)"
        );
        assert_eq!(
            undo_history[1].description(),
            "TestArcCommand: Increment(1)"
        );
    }

    #[test]
    fn test_redo_history() {
        let history = ConcurrentCommandHistory::new(NonZeroUsize::new(5).unwrap(), true);
        let ctx = SharedContext::new(TestArcContext { value: 0 });
        let command1 = TestArcCommand {
            operation: TestOperation::Increment(1),
        };
        let command2 = TestArcCommand {
            operation: TestOperation::Increment(2),
        };

        history.execute_command(command1, &ctx);
        history.execute_command(command2, &ctx);
        history.undo(&ctx);
        history.undo(&ctx);

        let redo_history = history.redo_history().unwrap();
        assert_eq!(redo_history.len(), 2);
        assert_eq!(
            redo_history[0].description(),
            "TestArcCommand: Increment(1)"
        );
        assert_eq!(
            redo_history[1].description(),
            "TestArcCommand: Increment(2)"
        );
    }
}
