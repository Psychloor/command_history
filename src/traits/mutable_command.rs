use std::borrow::Cow;

/// A trait representing a mutable command that can be executed, undone, and redone.
///
/// # Associated Types
///
/// * `Context`: The type of the context in which the command operates.
///
/// # Required Methods
///
/// * `execute(&self, ctx: &mut Self::Context)`: Executes the command in the given context.
/// * `undo(&self, ctx: &mut Self::Context)`: Undoes the command in the given context.
///
/// # Provided Methods
///
/// * `redo(&self, ctx: &mut Self::Context)`: Redoes the command by calling `execute` again. This method can be overridden if needed.
/// * `description(&self) -> Cow<str>`: Returns a description of the command. The default implementation returns "Unknown command".
pub trait MutableCommand {
    type Context;

    /// Executes the command in the given context.
    ///
    /// # Arguments
    ///
    /// * `ctx` - A mutable reference to the context in which the command operates.
    fn execute(&self, ctx: &mut Self::Context);

    /// Undoes the command in the given context.
    ///
    /// # Arguments
    ///
    /// * `ctx` - A mutable reference to the context in which the command operates.
    fn undo(&self, ctx: &mut Self::Context);

    /// Redoes the command by calling `execute` again. This method can be overridden if needed.
    ///
    /// # Arguments
    ///
    /// * `ctx` - A mutable reference to the context in which the command operates.
    fn redo(&self, ctx: &mut Self::Context) {
        self.execute(ctx);
    }

    /// Returns a description of the command. The default implementation returns "Unknown command".
    ///
    /// # Returns
    ///
    /// A string slice that holds the description of the command.
    fn description(&self) -> Cow<'_, str> {
        Cow::Borrowed("Unknown command")
    }
}
