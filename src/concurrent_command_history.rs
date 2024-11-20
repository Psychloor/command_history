use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};

use parking_lot::Mutex;

use crate::traits::{command::Command, command_history::CommandHistory};

pub struct ConcurrentCommandHistory<C: Command + Send + Sync> {
    undo: Mutex<VecDeque<C>>,
    redo: Mutex<VecDeque<C>>,
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
            undo: Mutex::new(VecDeque::with_capacity(limit)),
            redo: Mutex::new(VecDeque::with_capacity(limit)),
            history_limit: AtomicUsize::new(limit),
            clear_redo_on_execute: AtomicBool::new(clear_redo_on_execute),
        })
    }

    fn push_undo(&self, command: C, undo_lock: &mut parking_lot::MutexGuard<VecDeque<C>>) {
        let limit = self.history_limit.load(Ordering::Relaxed);
        while undo_lock.len() >= limit {
            undo_lock.pop_back();
        }

        undo_lock.push_front(command);
    }

    fn push_redo(&self, command: C, redo_lock: &mut parking_lot::MutexGuard<VecDeque<C>>) {
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
        command.execute(ctx);

        let mut undo = self.undo.lock();
        self.push_undo(command, &mut undo);

        if self.clear_redo_on_execute.load(Ordering::Relaxed) {
            self.redo.lock().clear();
        }
    }

    fn batch_execute(&self, commands: Vec<C>, ctx: &C::Context) {
        let mut undo = self.undo.lock();
        for command in commands {
            command.execute(ctx);

            self.push_undo(command, &mut undo);
        }

        if self.clear_redo_on_execute.load(Ordering::Relaxed) {
            self.redo.lock().clear();
        }
    }

    fn undo(&self, ctx: &C::Context) {
        let mut undo = self.undo.lock();
        if let Some(command) = undo.pop_front() {
            command.undo(ctx);

            let mut redo = self.redo.lock();
            self.push_redo(command, &mut redo);
        }
    }

    fn redo(&self, ctx: &C::Context) {
        let mut redo = self.redo.lock();
        if let Some(command) = redo.pop_front() {
            command.redo(ctx);

            let mut undo = self.undo.lock();
            self.push_undo(command, &mut undo);
        }
    }

    fn set_history_limit(&self, limit: NonZeroUsize) {
        assert!(limit.get() > 0);
        let limit = limit.get();

        self.history_limit.store(limit, Ordering::Release);

        let mut undo = self.undo.lock();
        while undo.len() > limit {
            undo.pop_back();
        }

        let mut redo = self.redo.lock();
        while redo.len() > limit {
            redo.pop_back();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cmp::min, hint::black_box, num::NonZero, panic::AssertUnwindSafe, thread};

    use rand::Rng;

    use crate::shared_context::SharedContext;

    use super::*;

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
        assert_eq!(history.undo.lock().len(), 1);
        assert!(history.redo.lock().is_empty());

        history.execute_command(command2, &ctx);
        assert_eq!(ctx.lock().value, 3);
        assert_eq!(history.undo.lock().len(), 2);
        assert!(history.redo.lock().is_empty());

        history.execute_command(command3, &ctx);

        assert_eq!(ctx.lock().value, 6);
        assert_eq!(history.undo.lock().len(), 2);
        assert!(history.redo.lock().is_empty());
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
        assert_eq!(history.undo.lock().len(), min(size, 35));
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
        assert_eq!(history.undo.lock().len(), 1);
        assert!(history.redo.lock().is_empty());

        history.execute_command(command2, &ctx);
        assert_eq!(ctx.lock().value, 3);
        assert_eq!(history.undo.lock().len(), 2);
        assert!(history.redo.lock().is_empty());

        history.execute_command(command3, &ctx);
        assert_eq!(ctx.lock().value, 6);
        assert_eq!(history.undo.lock().len(), 2);
        assert!(history.redo.lock().is_empty());

        history.set_history_limit(NonZero::new(1).unwrap());
        assert_eq!(history.undo.lock().len(), 1);
        assert!(history.redo.lock().is_empty());
    }

    #[test]
    fn test_arc_concurrent_rng() {
        let size = rand::thread_rng().gen_range(3..80);
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

            handles.push(black_box(std::thread::spawn(move || {
                thread::sleep(std::time::Duration::from_millis(
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
        assert_eq!(history.undo.lock().len(), min(size, count));
        assert!(history.redo.lock().is_empty());

        let mut handles = Vec::new();
        for _ in 0..size {
            let history_clone = Arc::clone(&history);
            let ctx_clone = ctx.clone();

            handles.push(black_box(std::thread::spawn(move || {
                thread::sleep(std::time::Duration::from_millis(
                    rand::thread_rng().gen_range(0..200),
                ));
                history_clone.undo(&ctx_clone);
                thread::sleep(std::time::Duration::from_millis(
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
            history.undo.lock().len(),
            min(size, count),
            "Undo stack size after execution"
        );
        assert!(
            history.redo.lock().is_empty(),
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
        assert_eq!(history.undo.lock().len(), 1);
        assert!(history.redo.lock().is_empty());

        history.undo(&ctx);
        assert_eq!(ctx.lock().value, 0);
        assert!(history.undo.lock().is_empty());
        assert_eq!(history.redo.lock().len(), 1);

        history.execute_command(command2, &ctx);
        assert_eq!(ctx.lock().value, 1);
        assert_eq!(history.undo.lock().len(), 1);
        assert!(history.redo.lock().is_empty());
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
        assert_eq!(history.undo.lock().len(), 1);
        assert!(history.redo.lock().is_empty());

        history.undo(&ctx);
        assert_eq!(ctx.lock().value, 0);
        assert!(history.undo.lock().is_empty());
        assert_eq!(history.redo.lock().len(), 1);

        history.execute_command(command2, &ctx);
        assert_eq!(ctx.lock().value, 1);
        assert_eq!(history.undo.lock().len(), 1);
        assert_eq!(history.redo.lock().len(), 1);
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
}
