🎯 **What:** The code health issue addressed was an overly long function, `tools_json`, which contained a massive, hardcoded JSON string literal representing the base tools. This code was refactored to construct the `Vec<Value>` programmatically using the `json!` macro within a new helper function `base_tools()`.

💡 **Why:** Breaking down the monolithic string literal into individual tool definitions via the `json!` macro improves code readability, structure, and maintainability. It is now much easier to read, add, or modify individual tools without needing to sift through a single gigantic JSON string block.

✅ **Verification:** I confirmed the change by running the existing test suite (`cargo test`), ensuring all 31 tests passed without any issues. The `git diff` also verified that the formatting correctly utilized rustfmt and the functional behavior of serializing into an identical array structure remains preserved.

✨ **Result:** The `tools_json` function is now much shorter and clearer, only concerning itself with the concatenation and serialization, while the tool structure definition is abstracted away into cleanly defined elements using `json!` objects within the `base_tools()` helper function.
