/// A trait representing a command that can be executed, undone, and redone within a given context.
///
/// This trait defines the basic operations for a command, including execution, undoing, and redoing.
/// It also provides a method to get a description of the command.
///
/// # Associated Types
///
/// * `Context`: The type of the context in which the command operates.
///
/// # Required Methods
///
/// * `execute(&self, ctx: &Self::Context)`: Executes the command with the given context.
/// * `undo(&self, ctx: &Self::Context)`: Undoes the command with the given context.
///
/// # Provided Methods
///
/// * `redo(&self, ctx: &Self::Context)`: Redoes the command by calling `execute`. This method can be overridden if needed.
/// * `description(&self) -> &str`: Returns a description of the command. The default implementation returns "Unknown command".
///
/// # Example
///
/// ```
/// use command_history::prelude::Command;
///
/// struct MyCommand;
///
/// impl Command for MyCommand {
///     type Context = ();
///
///     fn execute(&self, _ctx: &Self::Context) {
///         println!("Executing command");
///     }
///
///     fn undo(&self, _ctx: &Self::Context) {
///         println!("Undoing command");
///     }
///
///     fn description(&self) -> &str {
///         "My custom command"
///     }
/// }
///
/// let cmd = MyCommand;
/// cmd.execute(&());
/// println!("{}", cmd.description());
/// cmd.undo(&());
/// ```
pub trait Command {
    type Context;

    /// Executes the command with the given context.
    ///
    /// # Arguments
    ///
    /// * `ctx`: A reference to the context in which the command operates.
    fn execute(&self, ctx: &Self::Context);
    /// Undoes the command with the given context.
    ///
    /// # Arguments
    ///
    /// * `ctx`: A reference to the context in which the command operates.
    fn undo(&self, ctx: &Self::Context);

    /// Redoes the command by calling `execute`. This method can be overridden if needed.
    ///
    /// # Arguments
    ///
    /// * `ctx`: A reference to the context in which the command operates.
    fn redo(&self, ctx: &Self::Context) {
        self.execute(ctx);
    }

    /// Returns a description of the command. The default implementation returns "Unknown command".
    ///
    /// # Returns
    ///
    /// A string slice that holds the description of the command.
    fn description(&self) -> &str {
        "Unknown command"
    }
}
