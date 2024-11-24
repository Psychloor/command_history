use std::{collections::VecDeque, num::NonZeroUsize};

use crate::traits::{
    mutable_command::MutableCommand, mutable_command_history::MutableCommandHistory,
};

pub struct SimpleCommandHistory<C: MutableCommand> {
    undo: VecDeque<C>,
    redo: VecDeque<C>,
    history_limit: usize,
    clear_redo_on_execute: bool,
}

impl<C: MutableCommand> SimpleCommandHistory<C> {
    #[must_use]
    pub fn new(history_limit: usize, clear_redo_on_execute: bool) -> Self {
        Self {
            undo: VecDeque::with_capacity(history_limit),
            redo: VecDeque::with_capacity(history_limit),
            history_limit,
            clear_redo_on_execute,
        }
    }
    #[must_use]
    pub fn undo_history(&self) -> Option<Vec<&C>> {
        if self.undo.is_empty() {
            None
        } else {
            Some(self.undo.iter().collect())
        }
    }

    #[must_use]
    pub fn redo_history(&self) -> Option<Vec<&C>> {
        if self.redo.is_empty() {
            None
        } else {
            Some(self.redo.iter().collect())
        }
    }

    fn push_undo(&mut self, command: C) {
        while self.undo.len() >= self.history_limit {
            self.undo.pop_back();
        }

        self.undo.push_front(command);
    }

    fn push_redo(&mut self, command: C) {
        while self.redo.len() >= self.history_limit {
            self.redo.pop_back();
        }

        self.redo.push_front(command);
    }
}

impl<C: MutableCommand> MutableCommandHistory<C> for SimpleCommandHistory<C> {
    fn execute_command(&mut self, command: C, ctx: &mut C::Context) {
        command.execute(ctx);

        self.push_undo(command);

        if self.clear_redo_on_execute {
            self.redo.clear();
        }
    }

    fn undo(&mut self, ctx: &mut C::Context) {
        if let Some(command) = self.undo.pop_front() {
            command.undo(ctx);

            self.push_redo(command);
        }
    }

    fn redo(&mut self, ctx: &mut C::Context) {
        if let Some(command) = self.redo.pop_front() {
            command.execute(ctx);
            self.push_undo(command);
        }
    }

    fn set_history_limit(&mut self, limit: NonZeroUsize) {
        self.history_limit = limit.get();

        while self.undo.len() > self.history_limit {
            self.undo.pop_back();
        }

        while self.redo.len() > self.history_limit {
            self.redo.pop_back();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct TestCommand {
        value: i32,
    }

    impl MutableCommand for TestCommand {
        type Context = RefCell<i32>;
        fn execute(&self, ctx: &mut Self::Context) {
            *ctx.get_mut() += self.value;
        }

        fn undo(&self, ctx: &mut Self::Context) {
            *ctx.get_mut() -= self.value;
        }
    }

    #[test]
    fn test_new() {
        let history = SimpleCommandHistory::<TestCommand>::new(5, true);

        assert!(history.undo.is_empty());
        assert!(history.redo.is_empty());
        assert_eq!(history.history_limit, 5);
    }

    #[test]
    fn test_batch_execute() {
        let mut history = SimpleCommandHistory::new(2, true);
        let mut ctx = RefCell::new(0);
        let commands: Vec<_> = vec![
            TestCommand { value: 1 },
            TestCommand { value: 2 },
            TestCommand { value: 3 },
        ];

        history.batch_execute(commands, &mut ctx);
        assert_eq!(*ctx.borrow(), 6);
        assert_eq!(history.undo.len(), 2);
        assert!(history.redo.is_empty());

        history.undo(&mut ctx);
        assert_eq!(*ctx.borrow(), 3);
        assert_eq!(history.undo.len(), 1);
        assert_eq!(history.redo.len(), 1);
    }

    #[test]
    fn test_execute_command() {
        let mut history = SimpleCommandHistory::new(5, true);
        let mut ctx = RefCell::new(0);
        let command = TestCommand { value: 1 };

        history.execute_command(command, &mut ctx);

        assert_eq!(*ctx.borrow(), 1);
        assert_eq!(history.undo.len(), 1);
        assert!(history.redo.is_empty());
    }

    #[test]
    fn test_undo_command() {
        let mut history = SimpleCommandHistory::new(5, true);
        let mut ctx = RefCell::new(0);
        let command = TestCommand { value: 1 };

        history.execute_command(command, &mut ctx);
        assert_eq!(*ctx.borrow(), 1);
        assert_eq!(history.undo.len(), 1);
        assert!(history.redo.is_empty());

        history.undo(&mut ctx);

        assert_eq!(*ctx.borrow(), 0);
        assert!(history.undo.is_empty());
        assert_eq!(history.redo.len(), 1);
    }

    #[test]
    fn test_redo_command() {
        let mut history = SimpleCommandHistory::new(5, true);
        let mut ctx = RefCell::new(0);
        let command = TestCommand { value: 1 };
        let command1 = TestCommand { value: 1 };

        history.execute_command(command, &mut ctx);
        assert_eq!(*ctx.borrow(), 1);
        assert!(history.undo.len() == 1);
        assert!(history.redo.is_empty());

        history.undo(&mut ctx);
        assert_eq!(*ctx.borrow(), 0);
        assert!(history.undo.is_empty());
        assert_eq!(history.redo.len(), 1);

        history.redo(&mut ctx);

        assert_eq!(*ctx.borrow(), 1);
        assert_eq!(history.undo.len(), 1);
        assert!(history.redo.is_empty());

        history.undo(&mut ctx);
        assert_eq!(*ctx.borrow(), 0);
        assert!(history.undo.is_empty());
        assert_eq!(history.redo.len(), 1);

        history.execute_command(command1, &mut ctx);
        assert_eq!(*ctx.borrow(), 1);
        assert!(history.undo.len() == 1);
        assert!(history.redo.is_empty());
    }

    #[test]
    fn test_max_size() {
        let mut history = SimpleCommandHistory::new(5, true);
        let mut ctx = RefCell::new(0);

        for i in 1..=6 {
            let command = TestCommand { value: 1 };
            history.execute_command(command, &mut ctx);
            assert_eq!(*ctx.borrow(), i);
        }

        assert_eq!(*ctx.borrow(), 6);
        assert_eq!(history.undo.len(), 5);
        assert!(history.redo.is_empty());
    }

    #[test]
    fn test_set_history_limit() {
        let mut history = SimpleCommandHistory::new(5, true);
        let mut ctx = RefCell::new(0);

        for _ in 0..6 {
            let command = TestCommand { value: 1 };
            history.execute_command(command, &mut ctx);
        }

        assert_eq!(*ctx.borrow(), 6);
        assert_eq!(history.undo.len(), 5);
        assert!(history.redo.is_empty());

        history.set_history_limit(NonZeroUsize::new(3).unwrap());
        assert_eq!(*ctx.borrow(), 6);
        assert_eq!(history.undo.len(), 3);
        assert!(history.redo.is_empty());
    }

    #[test]
    fn test_dont_clear_redo() {
        let mut history = SimpleCommandHistory::new(5, false);
        let mut ctx = RefCell::new(0);

        for _ in 0..6 {
            let command = TestCommand { value: 1 };
            history.execute_command(command, &mut ctx);
        }

        assert_eq!(*ctx.borrow(), 6);
        assert_eq!(history.undo.len(), 5);
        assert!(history.redo.is_empty());

        history.undo(&mut ctx);
        history.undo(&mut ctx);

        assert_eq!(*ctx.borrow(), 4);
        assert_eq!(history.undo.len(), 3);
        assert_eq!(history.redo.len(), 2);

        history.execute_command(TestCommand { value: 1 }, &mut ctx);

        assert_eq!(*ctx.borrow(), 5);
        assert_eq!(history.undo.len(), 4);
        assert_eq!(history.redo.len(), 2);
    }

    #[test]
    fn test_clear_redo() {
        let mut history = SimpleCommandHistory::new(5, true);
        let mut ctx = RefCell::new(0);

        for _ in 0..6 {
            let command = TestCommand { value: 1 };
            history.execute_command(command, &mut ctx);
        }

        assert_eq!(*ctx.borrow(), 6);
        assert_eq!(history.undo.len(), 5);
        assert!(history.redo.is_empty());

        history.undo(&mut ctx);
        history.undo(&mut ctx);

        assert_eq!(*ctx.borrow(), 4);
        assert_eq!(history.undo.len(), 3);
        assert_eq!(history.redo.len(), 2);

        history.execute_command(TestCommand { value: 1 }, &mut ctx);

        assert_eq!(*ctx.borrow(), 5);
        assert_eq!(history.undo.len(), 4);
        assert!(history.redo.is_empty());
    }

    #[test]
    fn test_undo_with_empty_history() {
        let mut history = SimpleCommandHistory::<TestCommand>::new(5, true);
        let mut ctx = RefCell::new(0);

        history.undo(&mut ctx);

        assert_eq!(*ctx.borrow(), 0);
        assert!(history.undo.is_empty());
        assert!(history.redo.is_empty());
    }

    #[test]
    fn test_redo_with_empty_history() {
        let mut history = SimpleCommandHistory::<TestCommand>::new(5, true);
        let mut ctx = RefCell::new(0);

        history.redo(&mut ctx);

        assert_eq!(*ctx.borrow(), 0);
        assert!(history.undo.is_empty());
        assert!(history.redo.is_empty());
    }

    #[test]
    fn test_execute_command_with_full_undo_history() {
        let mut history = SimpleCommandHistory::new(2, true);
        let mut ctx = RefCell::new(0);

        history.execute_command(TestCommand { value: 1 }, &mut ctx);
        history.execute_command(TestCommand { value: 2 }, &mut ctx);
        history.execute_command(TestCommand { value: 3 }, &mut ctx);

        assert_eq!(*ctx.borrow(), 6);
        assert_eq!(history.undo.len(), 2);
        assert!(history.redo.is_empty());
    }

    #[test]
    fn test_undo_redo_multiple_commands() {
        let mut history = SimpleCommandHistory::new(5, true);
        let mut ctx = RefCell::new(0);

        let commands = vec![
            TestCommand { value: 1 },
            TestCommand { value: 2 },
            TestCommand { value: 3 },
        ];

        for command in commands {
            history.execute_command(command, &mut ctx);
        }

        assert_eq!(*ctx.borrow(), 6);
        assert_eq!(history.undo.len(), 3);
        assert!(history.redo.is_empty());

        history.undo(&mut ctx);
        assert_eq!(*ctx.borrow(), 3);
        assert_eq!(history.undo.len(), 2);
        assert_eq!(history.redo.len(), 1);

        history.redo(&mut ctx);
        assert_eq!(*ctx.borrow(), 6);
        assert_eq!(history.undo.len(), 3);
        assert!(history.redo.is_empty());
    }

    #[test]
    fn test_set_history_limit_with_existing_commands() {
        let mut history = SimpleCommandHistory::new(5, true);
        let mut ctx = RefCell::new(0);

        for _ in 0..5 {
            history.execute_command(TestCommand { value: 1 }, &mut ctx);
        }

        assert_eq!(*ctx.borrow(), 5);
        assert_eq!(history.undo.len(), 5);

        history.set_history_limit(NonZeroUsize::new(3).unwrap());
        assert_eq!(history.undo.len(), 3);
    }

    #[test]
    fn test_undo_history() {
        let mut history = SimpleCommandHistory::new(5, true);
        let mut ctx = RefCell::new(0);

        let commands = vec![
            TestCommand { value: 1 },
            TestCommand { value: 2 },
            TestCommand { value: 3 },
        ];

        for command in commands {
            history.execute_command(command, &mut ctx);
        }

        let undo_history = history.undo_history().unwrap();
        assert_eq!(undo_history.len(), 3);
        assert_eq!(undo_history[0].value, 3);
        assert_eq!(undo_history[1].value, 2);
        assert_eq!(undo_history[2].value, 1);
    }

    #[test]
    fn test_redo_history() {
        let mut history = SimpleCommandHistory::new(5, true);
        let mut ctx = RefCell::new(0);

        let commands = vec![
            TestCommand { value: 1 },
            TestCommand { value: 2 },
            TestCommand { value: 3 },
        ];

        for command in commands {
            history.execute_command(command, &mut ctx);
        }

        history.undo(&mut ctx);
        history.undo(&mut ctx);

        let redo_history = history.redo_history().unwrap();
        assert_eq!(redo_history.len(), 2);
        assert_eq!(redo_history[0].value, 2);
        assert_eq!(redo_history[1].value, 3);
    }

    #[test]
    fn test_undo_history_empty() {
        let history = SimpleCommandHistory::<TestCommand>::new(5, true);
        assert!(history.undo_history().is_none());
    }

    #[test]
    fn test_redo_history_empty() {
        let history = SimpleCommandHistory::<TestCommand>::new(5, true);
        assert!(history.redo_history().is_none());
    }
}