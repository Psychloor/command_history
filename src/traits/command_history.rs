use std::num::NonZeroUsize;

use super::command::Command;

pub trait CommandHistory<C: Command> {
    fn execute_command(&self, command: C, ctx: &C::Context);
    fn undo(&self, ctx: &C::Context);
    fn redo(&self, ctx: &C::Context);
    fn set_history_limit(&self, limit: NonZeroUsize);

    fn batch_execute(&self, commands: Vec<C>, ctx: &C::Context) {
        for command in commands {
            self.execute_command(command, ctx);
        }
    }
}
