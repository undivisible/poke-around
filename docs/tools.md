# Tools

Poke Gate exposes 7 tools to your Poke agent via MCP.

## run_command

Execute any shell command on your machine.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `command` | string | yes | The shell command to execute |
| `cwd` | string | no | Working directory (defaults to home) |

**Returns:** `{ stdout, stderr, exitCode }`

**Examples from Poke:**
- "Run `ls -la` in my home directory"
- "What's running on port 3000?"
- "Show me the git log for my project"
- "Install lodash in my project"

::: info
Commands have a 30-second timeout and 1MB output buffer.
:::

## read_file

Read the contents of a text file.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | yes | Absolute or relative path (supports `~`) |

**Examples:**
- "Read my ~/.zshrc"
- "Show me the package.json in my project"

## read_image

Read an image or binary file and return it as base64.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | yes | Path to the image file |

Supports: png, jpg, gif, webp, svg, pdf, bmp, ico.

For image files, returns MCP `image` content type with base64 data so the agent can "see" the image.

## write_file

Write content to a file. Creates the file if it doesn't exist, overwrites if it does.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | yes | Absolute or relative path (supports `~`) |
| `content` | string | yes | Content to write |

**Examples:**
- "Create a file called notes.txt on my Desktop"
- "Write a Python script that..."

## list_directory

List files and directories at a given path.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | no | Directory path (defaults to home, supports `~`) |

Returns entries with `d` for directories and `-` for files.

## system_info

Get system information. No parameters needed.

**Returns:**
```json
{
  "hostname": "MacBook-Pro.local",
  "platform": "darwin",
  "arch": "arm64",
  "uptime": "5h 23m",
  "totalMemory": "16GB",
  "freeMemory": "4GB",
  "homeDir": "/Users/you",
  "nodeVersion": "v22.21.1"
}
```

## take_screenshot

Capture the screen and save it to a file.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | no | Save path (defaults to `~/Desktop/screenshot-<timestamp>.png`) |

::: warning
Requires **Screen Recording** permission on macOS. The system will prompt you the first time.
:::
