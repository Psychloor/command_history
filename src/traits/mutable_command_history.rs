use std::num::NonZeroUsize;

use super::mutable_command::MutableCommand;

pub trait MutableCommandHistory<C: MutableCommand> {
    fn execute_command(&mut self, command: C, ctx: &mut C::Context);
    fn undo(&mut self, ctx: &mut C::Context);
    fn redo(&mut self, ctx: &mut C::Context);
    fn set_history_limit(&mut self, limit: NonZeroUsize);

    fn batch_execute(&mut self, commands: Vec<C>, ctx: &mut C::Context) {
        for command in commands {
            self.execute_command(command, ctx);
        }
    }
}
