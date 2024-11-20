# Command History Library

## Overview

This library provides a command history mechanism for managing and executing commands with undo and redo capabilities. It includes various modules and traits to facilitate the implementation of command patterns in Rust.

## Features

- **Simple Command History**: A straightforward implementation of command history with undo and redo functionality.
- **Concurrent Command History**: A thread-safe version of command history using `Arc` and `Mutex`.
- **Shared Context**: A utility for managing shared state across commands.
- **Traits**: Defines the necessary traits for commands and command histories.

## Modules

### `concurrent_command_history`
Provides a thread-safe implementation of command history.

### `shared_context`
Defines a shared context structure that can be used across multiple commands.

### `simple_command_history`
Implements a basic command history with undo and redo capabilities.

### `traits`
Contains the traits required for commands and command histories:
- `command`
- `command_history`
- `mutable_command`
- `mutable_command_history`

## Usage

Add the following to your `Cargo.toml`:

```toml
[dependencies]
command_history = { path = "path/to/command_history" }
```

### Example

```rust
use command_history::prelude::*;

struct MyCommand {
    // Command implementation
}

impl MutableCommand for MyCommand {
    // Implement required methods
}

fn main() {
    let mut history = SimpleCommandHistory::new(10, true);
    let mut ctx = RefCell::new(0);

    let command = MyCommand { /* fields */ };
    history.execute_command(command, &mut ctx);

    // Undo and redo operations
    history.undo_command(&mut ctx);
    history.redo_command(&mut ctx);
}
```

## Testing

The library includes comprehensive tests for all modules. To run the tests, use:

```sh
cargo test
```

## License

This project is licensed under either of:
- MIT license
- Apache License, Version 2.0

## Contributing

Contributions are welcome! Please open an issue or submit a pull request on GitHub.


