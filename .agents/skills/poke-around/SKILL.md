```markdown
# poke-around Development Patterns

> Auto-generated skill from repository analysis

## Overview
This skill teaches the core development patterns and conventions used in the `poke-around` Rust codebase. You'll learn about file naming, import/export styles, commit message practices, and how to structure and run tests. This guide is ideal for contributors looking to maintain consistency and quality in their code contributions.

## Coding Conventions

### File Naming
- Use **PascalCase** for file names.
  - Example: `GameLogic.rs`, `PlayerData.rs`

### Imports
- Use **relative imports** within modules.
  - Example:
    ```rust
    mod utils;
    use crate::utils::Helper;
    ```

### Exports
- Use **named exports** (i.e., `pub` items).
  - Example:
    ```rust
    pub struct GameState { /* ... */ }
    pub fn start_game() { /* ... */ }
    ```

### Commit Messages
- **Freeform** style, no strict prefixes.
- Average commit message length: ~51 characters.
  - Example: `Add initial player movement logic`

## Workflows

### Adding a New Module
**Trigger:** When you need to add a new feature or logical unit.
**Command:** `/add-module`

1. Create a new file using PascalCase (e.g., `InventoryManager.rs`).
2. Define your structs, enums, and functions with `pub` as needed.
3. Use relative imports to access other modules.
4. Export your main types/functions with `pub`.
5. Update the main module (`lib.rs` or `main.rs`) to include your new module.

### Writing and Running Tests
**Trigger:** When you need to test new or existing functionality.
**Command:** `/run-tests`

1. Create a test file matching the pattern `*.test.*` (e.g., `GameLogic.test.rs`).
2. Write your tests using Rust's built-in test framework (`#[cfg(test)]` and `#[test]`).
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_game_start() {
           assert_eq!(start_game(), true);
       }
   }
   ```
3. Run tests using `cargo test`.

### Committing Changes
**Trigger:** When you are ready to commit your work.
**Command:** `/commit-changes`

1. Write a descriptive commit message (no strict prefix required).
2. Keep the message concise (around 50 characters is typical).
3. Commit your changes using `git commit`.

## Testing Patterns

- Test files follow the pattern: `*.test.*` (e.g., `PlayerData.test.rs`).
- Tests use Rust's built-in testing framework.
- Place test modules inside the file or in separate test files as needed.
- Example:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_player_creation() {
          let player = Player::new("Ash");
          assert_eq!(player.name, "Ash");
      }
  }
  ```

## Commands
| Command         | Purpose                                   |
|-----------------|-------------------------------------------|
| /add-module     | Scaffold a new module with conventions    |
| /run-tests      | Run all tests in the codebase             |
| /commit-changes | Commit staged changes with best practices |
```