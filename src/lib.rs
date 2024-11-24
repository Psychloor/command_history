#![warn(clippy::pedantic)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![warn(clippy::correctness)]
#![warn(clippy::complexity)]
#![warn(clippy::suspicious)]
#![warn(clippy::cargo)]
#![allow(dead_code)]

pub mod concurrent_command_history;
pub mod shared_context;
pub mod simple_command_history;
pub mod traits;

pub mod prelude {
	pub use crate::concurrent_command_history::ConcurrentCommandHistory;
	pub use crate::shared_context::SharedContext;
	pub use crate::simple_command_history::SimpleCommandHistory;
	pub use crate::traits::command::Command;
	pub use crate::traits::command_history::CommandHistory;
	pub use crate::traits::mutable_command::MutableCommand;
	pub use crate::traits::mutable_command_history::MutableCommandHistory;
}