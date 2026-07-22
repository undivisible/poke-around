use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once, OnceLock, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use crate::mcp::AppState;
use crate::mcp::{
    block_private_urls, error_result, int_arg, ok_json, ok_json_with_image, ok_text,
    optional_output_path, path_arg, str_arg,
};
use crate::{Error, Result};
use fs2::FileExt;
use praefectus::CancellationToken;
use rs_peekaboo::{ImageCapture, ImageMode, Peekaboo, PeekabooConfig};

const MAX_FILE_READ_BYTES: u64 = 1024 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 1024;
const MAX_ARTIFACT_BYTES: u64 = 20 * 1024 * 1024;
const MAX_ARTIFACT_COUNT: usize = 16;
const MAX_ARTIFACT_AGE: Duration = Duration::from_secs(5 * 60);
static ARTIFACT_CACHE_LOCK: Mutex<()> = Mutex::new(());
static ARTIFACT_EXPIRATIONS: OnceLock<Mutex<HashMap<PathBuf, SystemTime>>> = OnceLock::new();
static ARTIFACT_JANITOR: Once = Once::new();
static IMAGE_CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);

// ponytail: single registry for all tools. Adding a tool = 1 entry, not 5 scattered lists.
#[derive(Clone, Copy)]
struct ToolDef {
    name: &'static str,
    handler: fn(&Value, &AppState) -> Result<Value>,
    approval: ApprovalCategory,
    summary: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApprovalCategory {
    None,
    Always,
    MutatingHttp,
    PermissionGrant,
}

static TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "run_command",
        handler: |_, _| unavailable_shell(),
        approval: ApprovalCategory::Always,
        summary: "Run command",
    },
    ToolDef {
        name: "network_speed",
        handler: |a, _| network_speed(a),
        approval: ApprovalCategory::None,
        summary: "Test network speed",
    },
    ToolDef {
        name: "read_file",
        handler: |a, s| read_file(a, s),
        approval: ApprovalCategory::None,
        summary: "Read file",
    },
    ToolDef {
        name: "write_file",
        handler: |a, s| write_file(a, s),
        approval: ApprovalCategory::Always,
        summary: "Write file",
    },
    ToolDef {
        name: "list_directory",
        handler: |a, s| list_directory(a, s),
        approval: ApprovalCategory::None,
        summary: "List directory",
    },
    ToolDef {
        name: "system_info",
        handler: |_, s| system_info(s),
        approval: ApprovalCategory::None,
        summary: "Get system info",
    },
    ToolDef {
        name: "read_image",
        handler: |a, s| read_image(a, s),
        approval: ApprovalCategory::Always,
        summary: "Read image file",
    },
    ToolDef {
        name: "run_agent",
        handler: |a, _| run_agent(a),
        approval: ApprovalCategory::Always,
        summary: "Run agent",
    },
    ToolDef {
        name: "take_screenshot",
        handler: |a, _| take_screenshot(a),
        approval: ApprovalCategory::Always,
        summary: "Take screenshot",
    },
    ToolDef {
        name: "edit_file",
        handler: |a, s| edit_file(a, s),
        approval: ApprovalCategory::Always,
        summary: "Edit file",
    },
    ToolDef {
        name: "web_fetch",
        handler: |a, _| web_fetch(a),
        approval: ApprovalCategory::None,
        summary: "Fetch URL",
    },
    ToolDef {
        name: "http_request",
        handler: |a, _| http_request(a),
        approval: ApprovalCategory::MutatingHttp,
        summary: "HTTP request",
    },
    ToolDef {
        name: "delete_file",
        handler: |a, s| delete_file(a, s),
        approval: ApprovalCategory::Always,
        summary: "Delete file",
    },
    ToolDef {
        name: "image",
        handler: |a, s| bounded_image(a, s),
        approval: ApprovalCategory::Always,
        summary: "Capture screen image",
    },
    ToolDef {
        name: "see",
        handler: |a, s| see(a, s),
        approval: ApprovalCategory::Always,
        summary: "Capture image with snapshot",
    },
    ToolDef {
        name: "list_screens",
        handler: |a, _| list_screens(a),
        approval: ApprovalCategory::None,
        summary: "List screens",
    },
    ToolDef {
        name: "permissions",
        handler: |a, _| permissions(a),
        approval: ApprovalCategory::PermissionGrant,
        summary: "Check permissions",
    },
    ToolDef {
        name: "doctor",
        handler: |_, _| doctor(),
        approval: ApprovalCategory::None,
        summary: "Computer-use health report",
    },
    ToolDef {
        name: "observe_ui",
        handler: |_, _| unavailable_target_effect(),
        approval: ApprovalCategory::Always,
        summary: "Observe semantic UI elements",
    },
    ToolDef {
        name: "click",
        handler: |_, _| unavailable_target_effect(),
        approval: ApprovalCategory::Always,
        summary: "Click on screen",
    },
    ToolDef {
        name: "press",
        handler: |_, _| unavailable_target_effect(),
        approval: ApprovalCategory::Always,
        summary: "Press key",
    },
    ToolDef {
        name: "type",
        handler: |_, _| unavailable_target_effect(),
        approval: ApprovalCategory::Always,
        summary: "Type text",
    },
    ToolDef {
        name: "paste",
        handler: |_, _| unavailable_target_effect(),
        approval: ApprovalCategory::Always,
        summary: "Paste text",
    },
    ToolDef {
        name: "hotkey",
        handler: |_, _| unavailable_target_effect(),
        approval: ApprovalCategory::Always,
        summary: "Press hotkey",
    },
    ToolDef {
        name: "scroll",
        handler: |_, _| unavailable_target_effect(),
        approval: ApprovalCategory::Always,
        summary: "Scroll screen",
    },
    ToolDef {
        name: "move",
        handler: |_, _| unavailable_target_effect(),
        approval: ApprovalCategory::Always,
        summary: "Move pointer",
    },
    ToolDef {
        name: "set_value",
        handler: |_, _| unavailable_target_effect(),
        approval: ApprovalCategory::Always,
        summary: "Set UI element value",
    },
    ToolDef {
        name: "perform_action",
        handler: |_, _| unavailable_target_effect(),
        approval: ApprovalCategory::Always,
        summary: "Perform UI action",
    },
    ToolDef {
        name: "window",
        handler: |_, _| unavailable_target_effect(),
        approval: ApprovalCategory::Always,
        summary: "Manage window",
    },
    ToolDef {
        name: "app",
        handler: |_, _| unavailable_target_effect(),
        approval: ApprovalCategory::Always,
        summary: "Manage app",
    },
    ToolDef {
        name: "open",
        handler: |a, s| open_target(a, s),
        approval: ApprovalCategory::Always,
        summary: "Open target",
    },
    ToolDef {
        name: "menu",
        handler: |_, _| unavailable_target_effect(),
        approval: ApprovalCategory::Always,
        summary: "Click menu item",
    },
    ToolDef {
        name: "clipboard_read",
        handler: |_, _| clipboard_read(),
        approval: ApprovalCategory::Always,
        summary: "Read clipboard",
    },
    ToolDef {
        name: "clipboard_write",
        handler: |a, _| clipboard_write(a),
        approval: ApprovalCategory::Always,
        summary: "Write clipboard",
    },
    ToolDef {
        name: "run",
        handler: |_, _| unavailable_automation_file(),
        approval: ApprovalCategory::Always,
        summary: "Run automation script",
    },
    ToolDef {
        name: "sleep",
        handler: |a, _| sleep_cmd(a),
        approval: ApprovalCategory::None,
        summary: "Sleep",
    },
    ToolDef {
        name: "clean",
        handler: |_, _| unavailable_multi_delete(),
        approval: ApprovalCategory::Always,
        summary: "Clean snapshots",
    },
];

pub fn execute_tool(tool_name: &str, args: &Value, state: &AppState) -> Result<Value> {
    if is_unavailable_tool(tool_name) {
        return Ok(error_result(
            "tool unavailable: hardened authority, privacy, or cancellation guarantees are not supported",
        ));
    }
    match find_tool(tool_name) {
        Some(tool) => (tool.handler)(args, state),
        None => Ok(error_result(format!("Unknown tool: {tool_name}"))),
    }
}

pub(crate) fn execute_tool_with_control(
    tool_name: &str,
    args: &Value,
    state: &AppState,
    cancellation: &CancellationToken,
    deadline: Instant,
) -> Result<Value> {
    if cancellation.is_cancelled() {
        return Ok(controlled_terminal("CANCELLED_BEFORE_EFFECT", true));
    }
    if Instant::now() >= deadline {
        return Ok(controlled_terminal("EXPIRED_BEFORE_EFFECT", true));
    }
    let result = if tool_name == "image" {
        execute_image_with_control(args, state, cancellation, deadline)
    } else {
        execute_tool(tool_name, args, state)
    };
    if cancellation.is_cancelled() || Instant::now() >= deadline {
        return Ok(controlled_terminal("OUTCOME_UNKNOWN", false));
    }
    result
}

fn execute_image_with_control(
    args: &Value,
    state: &AppState,
    cancellation: &CancellationToken,
    deadline: Instant,
) -> Result<Value> {
    if IMAGE_CAPTURE_ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(error_result("image capture already in progress"));
    }
    let args = args.clone();
    let state = state.clone();
    let worker_cancellation = cancellation.clone();
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = thread::Builder::new().spawn(move || {
        let _slot = ImageCaptureSlot;
        let result = bounded_image_with_control(&args, &state, &worker_cancellation, deadline);
        let _ = sender.send(result);
    });
    if let Err(error) = worker {
        IMAGE_CAPTURE_ACTIVE.store(false, Ordering::Release);
        return Err(error.into());
    }
    loop {
        if cancellation.is_cancelled() || Instant::now() >= deadline {
            return Ok(controlled_terminal("OUTCOME_UNKNOWN", false));
        }
        match receiver.recv_timeout(Duration::from_millis(25)) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Ok(controlled_terminal("OUTCOME_UNKNOWN", false));
            }
        }
    }
}

struct ImageCaptureSlot;

impl Drop for ImageCaptureSlot {
    fn drop(&mut self) {
        IMAGE_CAPTURE_ACTIVE.store(false, Ordering::Release);
    }
}

fn controlled_terminal(status: &str, retry_safe: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": status }],
        "structuredContent": { "status": status, "retry_safe": retry_safe },
        "isError": true
    })
}

pub(crate) fn tool_approval(tool_name: &str) -> Option<ApprovalCategory> {
    find_tool(tool_name).map(|t| t.approval)
}

pub(crate) fn tool_summary(tool_name: &str, args: &Value) -> String {
    match tool_name {
        "run_command" => {
            let command = str_arg(args, "command").unwrap_or("");
            format!(
                "Run executable '{}' from '{}' ({} chars, sha256:{})",
                command_executable(command),
                normalized_path_label(str_arg(args, "cwd").unwrap_or(".")),
                command.chars().count(),
                value_hash(command)
            )
        }
        "run_agent" => format!(
            "Run agent '{}'",
            bounded_label(str_arg(args, "name").unwrap_or("unknown"))
        ),
        "http_request" => format!(
            "HTTP {} to {}",
            bounded_label(str_arg(args, "method").unwrap_or("GET")),
            url_origin(str_arg(args, "url").unwrap_or(""))
        ),
        "write_file" => format!(
            "Write file '{}' ({} chars)",
            normalized_path_label(str_arg(args, "path").unwrap_or("")),
            text_length(args, "content")
        ),
        "edit_file" => format!(
            "Edit file '{}' ({} chars to {} chars)",
            normalized_path_label(str_arg(args, "path").unwrap_or("")),
            text_length(args, "old_string"),
            text_length(args, "new_string")
        ),
        "delete_file" => format!(
            "Delete file '{}'",
            normalized_path_label(str_arg(args, "path").unwrap_or(""))
        ),
        "take_screenshot" => format!(
            "Take screenshot to '{}'",
            str_arg(args, "path").map_or("temporary".to_string(), file_basename)
        ),
        "image" | "see" => format!(
            "{} {}{}",
            if tool_name == "see" {
                "Observe"
            } else {
                "Capture"
            },
            bounded_label(str_arg(args, "mode").unwrap_or("screen")),
            str_arg(args, "app")
                .map_or(String::new(), |app| format!(" in '{}'", bounded_label(app)))
        ),
        "click" => format!(
            "Click element reference with {} button {} time(s)",
            bounded_label(str_arg(args, "button").unwrap_or("left")),
            int_arg(args, "count").unwrap_or(1).max(1)
        ),
        "press" => format!(
            "Press key '{}' {} time(s)",
            bounded_label(str_arg(args, "key").unwrap_or("unknown")),
            int_arg(args, "count").unwrap_or(1).max(1)
        ),
        "type" => format!(
            "Type {} chars{}",
            text_length(args, "text"),
            str_arg(args, "app")
                .map_or(String::new(), |app| format!(" in '{}'", bounded_label(app)))
        ),
        "paste" => format!("Paste {} chars", text_length(args, "text")),
        "hotkey" => format!(
            "Press hotkey with {} key(s)",
            str_arg(args, "keys")
                .unwrap_or("")
                .split('+')
                .filter(|key| !key.trim().is_empty())
                .count()
        ),
        "scroll" => format!(
            "Scroll dx={} dy={}",
            int_arg(args, "dx").unwrap_or(0),
            int_arg(args, "dy").unwrap_or(0)
        ),
        "move" => "Move pointer to element reference".to_string(),
        "set_value" => format!("Set element value to {} chars", text_length(args, "value")),
        "perform_action" => format!(
            "Perform '{}' on element reference",
            bounded_label(str_arg(args, "action").unwrap_or("unknown"))
        ),
        "window" | "app" | "menu" => format!(
            "{} '{}'{}",
            if tool_name == "window" {
                "Window"
            } else if tool_name == "app" {
                "App"
            } else {
                "Menu"
            },
            bounded_label(str_arg(args, "action").unwrap_or("unknown")),
            str_arg(args, "app").map_or(String::new(), |app| format!(
                " for '{}'",
                bounded_label(app)
            ))
        ),
        "open" => format!(
            "Open {}{}",
            safe_open_target(str_arg(args, "target").unwrap_or("")),
            str_arg(args, "app").map_or(String::new(), |app| format!(
                " with '{}'",
                bounded_label(app)
            ))
        ),
        "clipboard_write" => format!("Write {} chars to clipboard", text_length(args, "text")),
        "run" => format!(
            "Run automation file '{}'",
            file_basename(str_arg(args, "file").unwrap_or(""))
        ),
        "clean" => "Clean snapshots".to_string(),
        _ => find_tool(tool_name)
            .map(|t| t.summary)
            .unwrap_or("Take screenshot")
            .to_string(),
    }
}

fn text_length(args: &Value, key: &str) -> usize {
    str_arg(args, key).map_or(0, |text| text.chars().count())
}

fn bounded_label(value: &str) -> String {
    value
        .chars()
        .take(64)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, ' ' | '.' | '_' | '-' | '+')
            {
                character
            } else {
                '?'
            }
        })
        .collect()
}

fn command_executable(command: &str) -> String {
    command
        .split_whitespace()
        .find(|part| !part.contains('='))
        .and_then(|part| Path::new(part.trim_matches(['\'', '"'])).file_name())
        .and_then(|name| name.to_str())
        .map(bounded_label)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn file_basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(bounded_label)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn normalized_path_label(path: &str) -> String {
    Path::new(path)
        .components()
        .collect::<std::path::PathBuf>()
        .to_string_lossy()
        .chars()
        .take(256)
        .map(|character| {
            if character.is_control() {
                '?'
            } else {
                character
            }
        })
        .collect()
}

fn value_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn url_origin(raw: &str) -> String {
    let Ok(url) = url::Url::parse(raw) else {
        return "invalid origin".to_string();
    };
    let Some(host) = url.host_str() else {
        return "invalid origin".to_string();
    };
    format!(
        "{}://{}{}",
        url.scheme(),
        host,
        url.port().map_or(String::new(), |port| format!(":{port}"))
    )
}

fn safe_open_target(target: &str) -> String {
    if target.starts_with("http://") || target.starts_with("https://") {
        url_origin(target)
    } else {
        format!("file '{}'", file_basename(target))
    }
}

fn find_tool(name: &str) -> Option<&'static ToolDef> {
    TOOLS.iter().find(|t| t.name == name)
}

pub fn tools_json() -> String {
    serde_json::to_string(&all_tool_schemas()).expect("tools json serializes")
}

fn all_tool_schemas() -> Vec<Value> {
    let mut tools = base_tools();
    tools.extend(computer_use_tools());
    tools.retain(|tool| {
        tool.get("name")
            .and_then(Value::as_str)
            .is_none_or(|name| !is_unavailable_tool(name))
    });
    for tool in &mut tools {
        let Some(name) = tool.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !matches!(
            tool_approval(name),
            Some(
                ApprovalCategory::Always
                    | ApprovalCategory::MutatingHttp
                    | ApprovalCategory::PermissionGrant
            )
        ) {
            continue;
        }
        if let Some(properties) = tool
            .get_mut("inputSchema")
            .and_then(|schema| schema.get_mut("properties"))
            .and_then(Value::as_object_mut)
        {
            properties.insert(
                "approval_request_id".to_string(),
                json!({
                    "type": "string",
                    "description": "Opaque request ID returned while awaiting local host approval",
                    "minLength": 32,
                    "maxLength": 32,
                    "pattern": "^[0-9a-f]{32}$"
                }),
            );
        }
    }
    tools
}

pub(crate) fn is_unavailable_tool(name: &str) -> bool {
    matches!(
        name,
        "run_command"
            | "network_speed"
            | "read_image"
            | "run_agent"
            | "take_screenshot"
            | "web_fetch"
            | "http_request"
            | "see"
            | "doctor"
            | "press"
            | "type"
            | "paste"
            | "hotkey"
            | "scroll"
            | "move"
            | "perform_action"
            | "window"
            | "app"
            | "open"
            | "menu"
            | "run"
            | "clean"
            | "sleep"
    )
}

fn base_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "run_command",
            "description": "Execute a shell command on the user's machine and return stdout, stderr, and exit code.",
            "inputSchema": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to execute"
                        },
                        "cwd": {
                            "type": "string",
                            "description": "Working directory (optional, defaults to home)"
                        },
                        "approval_request_id": { "type": "string" }
                    },
                    "required": [
                        "command"
                    ]
                }
        }),
        json!({
            "name": "network_speed",
            "description": "Run a built-in internet speed test and return download/upload Mbps.",
            "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tests": {
                            "type": "string",
                            "enum": [
                                "download",
                                "upload",
                                "both"
                            ],
                            "description": "Which direction to test"
                        }
                    }
                }
        }),
        json!({
            "name": "read_file",
            "description": "Read the contents of a file on the user's machine.",
            "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute or relative path to the file"
                        }
                    },
                    "required": [
                        "path"
                    ]
                }
        }),
        json!({
            "name": "write_file",
            "description": "Write content to a file on the user's machine.",
            "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute or relative path to the file"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write"
                        },
                        "approval_request_id": { "type": "string" }
                    },
                    "required": [
                        "path",
                        "content"
                    ]
                }
        }),
        json!({
            "name": "list_directory",
            "description": "List files and directories at a given path with sizes and types.",
            "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path (defaults to home)"
                        }
                    }
                }
        }),
        json!({
            "name": "system_info",
            "description": "Get system information: OS, hostname, architecture, uptime, memory.",
            "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
        }),
        json!({
            "name": "read_image",
            "description": "Read an image file and return MCP image content with metadata.",
            "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute or relative path to the image file"
                        }
                    },
                    "required": [
                        "path"
                    ]
                }
        }),
        json!({
            "name": "run_agent",
            "description": "Run a Poke Around agent by name.",
            "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Agent name"
                        }
                    },
                    "required": [
                        "name"
                    ]
                }
        }),
        json!({
            "name": "take_screenshot",
            "description": "Take a screenshot of the user's screen.",
            "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path to save the screenshot"
                        },
                        "approval_request_id": { "type": "string" }
                    }
                }
        }),
        json!({
            "name": "edit_file",
            "description": "Surgically replace an exact string in a file. Fails if old_string is not found or is ambiguous.",
            "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute or relative path to the file"
                        },
                        "old_string": {
                            "type": "string",
                            "description": "Exact string to replace"
                        },
                        "new_string": {
                            "type": "string",
                            "description": "Replacement string"
                        },
                        "approval_request_id": { "type": "string" }
                    },
                    "required": [
                        "path",
                        "old_string",
                        "new_string"
                    ]
                }
        }),
        json!({
            "name": "web_fetch",
            "description": "Fetch the text content of a URL and return it. Optionally truncate to max_chars.",
            "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "URL to fetch"
                        },
                        "max_chars": {
                            "type": "integer",
                            "description": "Maximum number of characters to return"
                        }
                    },
                    "required": [
                        "url"
                    ]
                }
        }),
        json!({
            "name": "http_request",
            "description": "Make an HTTP request with custom method, headers and body. Returns status code and response body.",
            "inputSchema": {
                    "type": "object",
                    "properties": {
                        "method": {
                            "type": "string",
                            "enum": [
                                "GET",
                                "POST",
                                "PUT",
                                "DELETE",
                                "PATCH",
                                "HEAD",
                                "OPTIONS"
                            ],
                            "description": "HTTP method"
                        },
                        "url": {
                            "type": "string",
                            "description": "URL to request"
                        },
                        "headers": {
                            "type": "object",
                            "description": "HTTP headers"
                        },
                        "body": {
                            "type": "string",
                            "description": "Request body"
                        }
                    },
                    "required": [
                        "method",
                        "url"
                    ]
                }
        }),
        json!({
            "name": "delete_file",
            "description": "Delete a file or empty directory. Requires approval for non-empty directories.",
            "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute or relative path to delete"
                        },
                        "approval_request_id": { "type": "string" }
                    },
                    "required": [
                        "path"
                    ]
                }
        }),
    ]
}

fn computer_use_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "observe_ui",
            "description": "Observe bounded semantic elements in the host-selected active UI. Returns short-lived tags, not screenshots or coordinates.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        }),
        json!({
            "name": "click",
            "description": "Semantically invoke one element from a current observe_ui result.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "observation_id": {
                        "type": "string",
                        "pattern": "^[0-9a-f]{64}$"
                    },
                    "generation": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 9007199254740991_u64
                    },
                    "tag": {
                        "type": "string",
                        "pattern": "^e(?:0|[1-9][0-9]{0,3})$"
                    },
                    "interaction_mode": {
                        "type": "string",
                        "enum": ["interactive", "background_only"]
                    }
                },
                "required": ["observation_id", "generation", "tag", "interaction_mode"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "set_value",
            "description": "Set the value of one editable element from a current observe_ui result.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "observation_id": {
                        "type": "string",
                        "pattern": "^[0-9a-f]{64}$"
                    },
                    "generation": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 9007199254740991_u64
                    },
                    "tag": {
                        "type": "string",
                        "pattern": "^e(?:0|[1-9][0-9]{0,3})$"
                    },
                    "value": {
                        "type": "string",
                        "maxLength": 4096
                    },
                    "interaction_mode": {
                        "type": "string",
                        "enum": ["interactive", "background_only"]
                    }
                },
                "required": ["observation_id", "generation", "tag", "value", "interaction_mode"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "image",
            "description": "Capture one bounded non-interactive screen image into the host-owned private artifact cache.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "retina": {"type": "boolean", "description": "Capture at retina scale"}
                },
                "additionalProperties": false
            }
        }),
        json!({
            "name": "see",
            "description": "Capture an image and cache a UI snapshot on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "app": {"type": "string", "description": "Optional app filter"},
                    "mode": {"type": "string", "description": "Capture mode: screen or window"},
                    "path": {"type": "string", "description": "Optional output path"},
                    "retina": {"type": "boolean", "description": "Capture at retina scale"}
                }
            }
        }),
        json!({
            "name": "list_screens",
            "description": "List display information for the user's machine.",
            "inputSchema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "permissions",
            "description": "Probe or grant screen recording, accessibility, and clipboard access.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "description": "Optional action: grant"}
                }
            }
        }),
        json!({
            "name": "doctor",
            "description": "Health report for computer-use readiness (permissions, tools, capabilities).",
            "inputSchema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "press",
            "description": "Press a named key on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Key name"},
                    "count": {"type": "number", "description": "Repeat count"},
                    "delay_ms": {"type": "number", "description": "Delay between repeats in milliseconds"}
                },
                "required": ["key"]
            }
        }),
        json!({
            "name": "type",
            "description": "Type text on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Text to type"},
                    "app": {"type": "string", "description": "Optional app name to focus before typing"},
                    "clear": {"type": "boolean", "description": "Clear the current field first"},
                    "return": {"type": "boolean", "description": "Press return after typing"},
                    "delay_ms": {"type": "number", "description": "Delay between characters in milliseconds"}
                },
                "required": ["text"]
            }
        }),
        json!({
            "name": "paste",
            "description": "Paste text into the active UI on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {"text": {"type": "string", "description": "Text to paste"}},
                "required": ["text"]
            }
        }),
        json!({
            "name": "hotkey",
            "description": "Press a hotkey on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {"keys": {"type": "string", "description": "Hotkey keys joined by plus signs"}},
                "required": ["keys"]
            }
        }),
        json!({
            "name": "scroll",
            "description": "Scroll on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dx": {"type": "number", "description": "Horizontal scroll amount"},
                    "dy": {"type": "number", "description": "Vertical scroll amount"}
                }
            }
        }),
        json!({
            "name": "window",
            "description": "List, focus, move, resize, minimize, close, or set bounds of windows on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "description": "Window action"},
                    "app": {"type": "string", "description": "Application name"},
                    "title": {"type": "string", "description": "Window title"},
                    "x": {"type": "number", "description": "Window x coordinate"},
                    "y": {"type": "number", "description": "Window y coordinate"},
                    "width": {"type": "number", "description": "Window width"},
                    "height": {"type": "number", "description": "Window height"}
                },
                "required": ["action"]
            }
        }),
        json!({
            "name": "app",
            "description": "List, launch, activate, switch, hide, unhide, or quit apps on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "description": "App action"},
                    "app": {"type": "string", "description": "Application name"}
                },
                "required": ["action"]
            }
        }),
        json!({
            "name": "open",
            "description": "Open a path or URL on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": {"type": "string", "description": "Path or URL to open"},
                    "app": {"type": "string", "description": "Optional app to open with"},
                    "no_focus": {"type": "boolean", "description": "Open without focusing the target app"}
                },
                "required": ["target"]
            }
        }),
        json!({
            "name": "menu",
            "description": "Inspect or click menu items on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "description": "Menu action"},
                    "app": {"type": "string", "description": "Application name"},
                    "menu": {"type": "string", "description": "Menu name"},
                    "item": {"type": "string", "description": "Menu item name"}
                },
                "required": ["action"]
            }
        }),
        json!({
            "name": "clipboard_read",
            "description": "Read the user's clipboard.",
            "inputSchema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "clipboard_write",
            "description": "Write text to the user's clipboard.",
            "inputSchema": {
                "type": "object",
                "properties": {"text": {"type": "string", "description": "Text to copy"}},
                "required": ["text"]
            }
        }),
        json!({
            "name": "sleep",
            "description": "Sleep for a number of seconds on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {"seconds": {"type": "number", "description": "Sleep duration in seconds"}},
                "required": ["seconds"]
            }
        }),
        json!({
            "name": "clean",
            "description": "Remove cached snapshots on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "all_snapshots": {"type": "boolean", "description": "Remove all cached snapshots"},
                    "snapshot": {"type": "string", "description": "Remove a specific snapshot id"}
                }
            }
        }),
    ]
}

// Tool handlers
fn peekaboo() -> Peekaboo {
    Peekaboo::with_config(PeekabooConfig {
        background: true,
        ..PeekabooConfig::default()
    })
}

fn bounded_image(args: &Value, state: &AppState) -> Result<Value> {
    bounded_image_with_control(
        args,
        state,
        &CancellationToken::default(),
        Instant::now() + Duration::from_secs(30),
    )
}

fn bounded_image_with_control(
    args: &Value,
    state: &AppState,
    cancellation: &CancellationToken,
    deadline: Instant,
) -> Result<Value> {
    if !valid_capture_args(args) {
        return Ok(error_result("invalid image arguments"));
    }
    if cancellation.is_cancelled() {
        return Ok(controlled_terminal("CANCELLED_BEFORE_EFFECT", true));
    }
    if Instant::now() >= deadline {
        return Ok(controlled_terminal("EXPIRED_BEFORE_EFFECT", true));
    }
    let retina = args.get("retina").and_then(Value::as_bool).unwrap_or(true);
    let staging = private_capture_path(state)?;
    let staging_guard = CapturePathGuard(staging.clone());
    let capture = peekaboo().image(ImageMode::Screen, Some(staging), retina)?;
    crate::config::restrict_private_file(&capture.path)?;
    if cancellation.is_cancelled() || Instant::now() >= deadline {
        return Ok(controlled_terminal("OUTCOME_UNKNOWN", false));
    }
    let metadata = json!({ "mode": capture.mode });
    let result = cache_image_artifact(metadata, &capture, state);
    drop(staging_guard);
    result
}

fn valid_capture_args(args: &Value) -> bool {
    let Some(object) = args.as_object() else {
        return false;
    };
    if !object
        .keys()
        .all(|key| matches!(key.as_str(), "retina" | "approval_request_id"))
    {
        return false;
    }
    if args.get("retina").is_some_and(|value| !value.is_boolean())
        || args.get("approval_request_id").is_some_and(|value| {
            !value.as_str().is_some_and(|value| {
                value.len() == 32
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
        })
    {
        return false;
    }
    true
}

struct CapturePathGuard(PathBuf);

impl Drop for CapturePathGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn private_capture_path(state: &AppState) -> Result<PathBuf> {
    let _process_guard = ARTIFACT_CACHE_LOCK
        .lock()
        .map_err(|_| Error::msg("image artifact cache unavailable"))?;
    let directory = artifact_cache_dir(&state.inner.home);
    fs::create_dir_all(&directory)?;
    crate::config::restrict_private_dir(&directory)?;
    let path = directory.join(format!(
        ".capture-{}-{:016x}.png",
        std::process::id(),
        rand::random::<u64>()
    ));
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    crate::config::restrict_private_file(&path)?;
    file.sync_all()?;
    Ok(path)
}

fn cache_image_artifact(
    metadata: Value,
    capture: &ImageCapture,
    state: &AppState,
) -> Result<Value> {
    let result = cache_image_artifact_inner(metadata, capture, state);
    let _ = fs::remove_file(&capture.path);
    result
}

fn cache_image_artifact_inner(
    mut metadata: Value,
    capture: &ImageCapture,
    state: &AppState,
) -> Result<Value> {
    if !capture.mime_type.starts_with("image/") || capture.mime_type.len() > 64 {
        return Err(Error::msg("image artifact has an invalid media type"));
    }
    let mut data = Vec::new();
    File::open(&capture.path)?
        .take(MAX_ARTIFACT_BYTES + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err(Error::msg("image artifact exceeds the 20 MiB limit"));
    }
    let digest = Sha256::digest(&data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let _process_guard = ARTIFACT_CACHE_LOCK
        .lock()
        .map_err(|_| Error::msg("image artifact cache unavailable"))?;
    let directory = artifact_cache_dir(&state.inner.home);
    fs::create_dir_all(&directory)?;
    crate::config::restrict_private_dir(&directory)?;
    let lock_path = directory.join("artifacts.lock");
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    crate::config::restrict_private_file(&lock_path)?;
    lock.lock_exclusive()?;
    let path = directory.join(&digest);
    if path.exists() {
        if bounded_file_digest(&path)? != digest {
            return Err(Error::msg("image artifact cache conflict"));
        }
    } else {
        let temporary = directory.join(format!(
            ".tmp-{}-{:016x}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&data)?;
            file.sync_all()?;
            crate::config::restrict_private_file(&temporary)?;
            fs::rename(&temporary, &path)?;
            Ok::<(), Error>(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
        crate::config::restrict_private_file(&path)?;
        sync_directory(&directory)?;
    }
    let modified = SystemTime::now();
    File::options()
        .write(true)
        .open(&path)?
        .set_times(fs::FileTimes::new().set_modified(modified))?;
    prune_artifact_cache(&directory, Some(&path))?;
    register_artifact_expiration(path.clone(), modified)?;
    let artifact = json!({
        "locator": format!("sha256:{digest}"),
        "sha256": digest,
        "bytes": data.len(),
        "mime_type": capture.mime_type,
    });
    metadata
        .as_object_mut()
        .ok_or_else(|| Error::msg("image metadata must be an object"))?
        .insert("artifact".to_string(), artifact);
    Ok(ok_json(metadata))
}

fn artifact_cache_dir(home: &Path) -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".cache"));
    base.join("poke-around").join("artifacts")
}

fn artifact_expirations() -> &'static Mutex<HashMap<PathBuf, SystemTime>> {
    ARTIFACT_EXPIRATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_artifact_expiration(path: PathBuf, modified: SystemTime) -> Result<()> {
    ARTIFACT_JANITOR.call_once(|| {
        thread::spawn(|| {
            loop {
                thread::sleep(Duration::from_secs(1));
                let now = SystemTime::now();
                let due = artifact_expirations()
                    .lock()
                    .map(|mut expirations| {
                        let due = expirations
                            .iter()
                            .filter_map(|(path, expires_at)| {
                                (*expires_at <= now).then_some(path.clone())
                            })
                            .collect::<Vec<_>>();
                        for path in &due {
                            expirations.remove(path);
                        }
                        due
                    })
                    .unwrap_or_default();
                for path in due {
                    let _ = expire_artifact(&path);
                }
            }
        });
    });
    let expires_at = modified.checked_add(MAX_ARTIFACT_AGE).unwrap_or(modified);
    artifact_expirations()
        .lock()
        .map_err(|_| Error::msg("image artifact cache unavailable"))?
        .insert(path, expires_at);
    Ok(())
}

fn expire_artifact(path: &Path) -> Result<()> {
    let Some(directory) = path.parent() else {
        return Ok(());
    };
    let _process_guard = ARTIFACT_CACHE_LOCK
        .lock()
        .map_err(|_| Error::msg("image artifact cache unavailable"))?;
    let lock_path = directory.join("artifacts.lock");
    let lock = match fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock_path)
    {
        Ok(lock) => lock,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    lock.lock_exclusive()?;
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    if SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default()
        < MAX_ARTIFACT_AGE
    {
        return register_artifact_expiration(path.to_path_buf(), modified);
    }
    fs::remove_file(path)?;
    sync_directory(directory)
}

fn bounded_file_digest(path: &Path) -> Result<String> {
    let mut data = Vec::new();
    File::open(path)?
        .take(MAX_ARTIFACT_BYTES + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err(Error::msg("image artifact cache conflict"));
    }
    Ok(Sha256::digest(&data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn prune_artifact_cache(directory: &Path, protected: Option<&Path>) -> Result<()> {
    let now = SystemTime::now();
    let mut artifacts = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let modified = entry
            .metadata()?
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.len() != 64
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            if (name.starts_with(".tmp-") || name.starts_with(".capture-"))
                && now.duration_since(modified).unwrap_or_default() > MAX_ARTIFACT_AGE
            {
                fs::remove_file(path)?;
            }
            continue;
        }
        crate::config::restrict_private_file(&path)?;
        if protected != Some(path.as_path())
            && now.duration_since(modified).unwrap_or_default() > MAX_ARTIFACT_AGE
        {
            fs::remove_file(path)?;
        } else {
            artifacts.push((modified, path));
        }
    }
    artifacts.sort_by_key(|(modified, path)| (*modified, path.clone()));
    while artifacts.len() > MAX_ARTIFACT_COUNT {
        let index = artifacts
            .iter()
            .position(|(_, path)| protected != Some(path.as_path()))
            .ok_or_else(|| Error::msg("image artifact cache unavailable"))?;
        let (_, path) = artifacts.remove(index);
        fs::remove_file(path)?;
    }
    sync_directory(directory)
}

pub(crate) fn harden_artifact_cache(home: &Path) -> Result<()> {
    let _process_guard = ARTIFACT_CACHE_LOCK
        .lock()
        .map_err(|_| Error::msg("image artifact cache unavailable"))?;
    let directory = artifact_cache_dir(home);
    if !directory.try_exists()? {
        return Ok(());
    }
    crate::config::restrict_private_dir(&directory)?;
    let lock_path = directory.join("artifacts.lock");
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    crate::config::restrict_private_file(&lock_path)?;
    lock.lock_exclusive()?;
    prune_artifact_cache(&directory, None)?;
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.file_type()?.is_file()
            && name.len() == 64
            && name
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            let modified = entry
                .metadata()?
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH);
            register_artifact_expiration(entry.path(), modified)?;
        }
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn see(args: &Value, _state: &AppState) -> Result<Value> {
    let app = str_arg(args, "app");
    let mode = ImageMode::parse_or_err(str_arg(args, "mode").unwrap_or("screen"))?;
    let path = optional_output_path(args)?;
    let retina = args.get("retina").and_then(Value::as_bool).unwrap_or(true);
    // see() assigns stable element indices and caches the snapshot.
    let snapshot = peekaboo().see(app, mode, path.clone(), retina)?;
    let snapshot_dir = rs_peekaboo::cache::snapshot_dir()?;
    crate::config::restrict_private_dir(&snapshot_dir)?;
    crate::config::restrict_private_file(
        &snapshot_dir.join(format!("{}.json", snapshot.snapshot_id)),
    )?;
    let capture = peekaboo().image(mode, path, retina)?;
    crate::config::restrict_private_file(&capture.path)?;
    let metadata = json!({
        "snapshot_id": snapshot.snapshot_id,
        "mode": capture.mode,
    });
    ok_json_with_image(metadata, &capture)
}

fn list_screens(_args: &Value) -> Result<Value> {
    Ok(ok_json(peekaboo().list_screens()?))
}

fn permissions(args: &Value) -> Result<Value> {
    if str_arg(args, "action") == Some("grant") {
        Ok(ok_json(peekaboo().grant_permissions()?))
    } else {
        Ok(ok_json(peekaboo().permissions()))
    }
}

fn doctor() -> Result<Value> {
    Ok(ok_json(peekaboo().doctor()?))
}

fn unavailable_target_effect() -> Result<Value> {
    Ok(error_result(
        "targeted computer-use effect unavailable: live target identity fencing is not supported",
    ))
}

fn open_target(args: &Value, _state: &AppState) -> Result<Value> {
    let target = str_arg(args, "target").unwrap_or("");
    let app = str_arg(args, "app");
    let no_focus = args
        .get("no_focus")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(ok_json(peekaboo().open(target, app, no_focus)?))
}

fn clipboard_read() -> Result<Value> {
    let text = peekaboo().clipboard_read()?;
    if text.len() as u64 > MAX_FILE_READ_BYTES {
        return Err(Error::msg("clipboard exceeds the 1 MiB read limit"));
    }
    Ok(ok_json(json!({ "text": text })))
}

fn clipboard_write(args: &Value) -> Result<Value> {
    let text = str_arg(args, "text").unwrap_or("");
    Ok(ok_json(peekaboo().clipboard_write(text)?))
}

fn unavailable_automation_file() -> Result<Value> {
    Ok(error_result(
        "automation scripts unavailable: command-level target fencing is not supported",
    ))
}

fn sleep_cmd(args: &Value) -> Result<Value> {
    let seconds = args.get("seconds").and_then(Value::as_f64).unwrap_or(0.0);
    let millis = (seconds.max(0.0) * 1000.0) as u64;
    std::thread::sleep(Duration::from_millis(millis));
    Ok(ok_json(json!({ "slept_ms": millis })))
}

fn unavailable_multi_delete() -> Result<Value> {
    Ok(error_result(
        "multi-file deletion unavailable: between-effect cancellation is not supported",
    ))
}

fn network_speed(args: &Value) -> Result<Value> {
    let tests = args.get("tests").and_then(Value::as_str).unwrap_or("both");
    let mut result = serde_json::Map::new();
    if matches!(tests, "download" | "both") {
        result.insert(
            "download_mbps".to_string(),
            json!(measure_curl_speed(&[
                "-o",
                "/dev/null",
                "-sS",
                "-w",
                "%{speed_download}",
                "https://speed.cloudflare.com/__down?bytes=10000000",
            ])?),
        );
    }
    if matches!(tests, "upload" | "both") {
        result.insert(
            "upload_mbps".to_string(),
            json!(measure_curl_speed(&[
                "-o",
                "/dev/null",
                "-sS",
                "-w",
                "%{speed_upload}",
                "-X",
                "POST",
                "--data-binary",
                "poke-around-speed-test",
                "https://speed.cloudflare.com/__up",
            ])?),
        );
    }
    Ok(ok_json(Value::Object(result)))
}

fn measure_curl_speed(args: &[&str]) -> Result<f64> {
    let output = Command::new("curl").args(args).output()?;
    if !output.status.success() {
        return Err(Error::msg(String::from_utf8_lossy(&output.stderr)));
    }
    let bytes_per_second = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .map_err(|err| Error::msg(format!("invalid curl speed output: {err}")))?;
    Ok((bytes_per_second * 8.0) / 1_000_000.0)
}

fn unavailable_shell() -> Result<Value> {
    Ok(error_result(
        "shell execution unavailable: reliable cross-platform process-tree cancellation is not supported",
    ))
}

fn read_file(args: &Value, state: &AppState) -> Result<Value> {
    let path = path_arg(args, state)?;
    Ok(ok_text(read_text_file_bounded(&path)?))
}

fn read_text_file_bounded(path: &Path) -> Result<String> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_FILE_READ_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_READ_BYTES {
        return Err(Error::msg("file exceeds the 1 MiB read limit"));
    }
    let text = String::from_utf8(bytes).map_err(|_| Error::msg("file is not valid UTF-8"))?;
    Ok(text)
}

fn write_file(args: &Value, state: &AppState) -> Result<Value> {
    let path = path_arg(args, state)?;
    let content = args.get("content").and_then(Value::as_str).unwrap_or("");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, content)?;
    Ok(ok_json(json!({ "path": path, "bytes": content.len() })))
}

fn list_directory(args: &Value, state: &AppState) -> Result<Value> {
    let path = path_arg(args, state)?;
    let mut entries = Vec::new();
    for (index, entry) in fs::read_dir(&path)?.enumerate() {
        if index >= MAX_DIRECTORY_ENTRIES {
            return Err(Error::msg("directory exceeds the 1024 entry limit"));
        }
        let entry = entry?;
        let file_type = entry.file_type()?;
        let is_dir = file_type.is_dir();
        let size = if is_dir { 0 } else { entry.metadata()?.len() };
        entries.push(json!({
            "name": entry.file_name().to_string_lossy(),
            "path": entry.path(),
            "is_dir": is_dir,
            "size": size
        }));
    }
    Ok(ok_json(json!({ "path": path, "entries": entries })))
}

fn command_output(command: &str, args: &[&str]) -> Option<String> {
    Command::new(command)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|text| !text.is_empty())
}

fn system_memory_bytes() -> Option<u64> {
    if cfg!(target_os = "macos") {
        command_output("sysctl", &["-n", "hw.memsize"])?
            .parse()
            .ok()
    } else if cfg!(target_os = "linux") {
        let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
        for line in meminfo.lines() {
            if let Some(value) = line.strip_prefix("MemTotal:") {
                let kb = value.split_whitespace().next()?.parse::<u64>().ok()?;
                return Some(kb * 1024);
            }
        }
        None
    } else {
        None
    }
}

fn system_info(state: &AppState) -> Result<Value> {
    Ok(ok_json(json!({
        "os": std::env::consts::OS,
        "hostname": command_output("hostname", &[]).unwrap_or_default(),
        "arch": std::env::consts::ARCH,
        "uptime": command_output("uptime", &[]).unwrap_or_default(),
        "memory_bytes": system_memory_bytes(),
        "home": state.inner.home,
        "now": SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs()
    })))
}

fn read_image(args: &Value, state: &AppState) -> Result<Value> {
    let path = path_arg(args, state)?;
    let mime = match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    };
    let metadata = json!({});
    let capture = ImageCapture {
        path: path.clone(),
        mode: ImageMode::Screen,
        bytes: fs::metadata(&path)?.len(),
        mime_type: mime.to_string(),
        ephemeral: false,
    };
    ok_json_with_image(metadata, &capture)
}

fn run_agent(args: &Value) -> Result<Value> {
    let name = args.get("name").and_then(Value::as_str).unwrap_or("");
    crate::agents::run_agent_by_name(name)?;
    Ok(ok_json(json!({ "name": name, "status": "ran" })))
}

// ponytail: take_screenshot == image(mode:"screen")
fn take_screenshot(args: &Value) -> Result<Value> {
    let mode = ImageMode::Screen;
    let path = optional_output_path(args)?;
    let capture = peekaboo().image(mode, path, true)?;
    crate::config::restrict_private_file(&capture.path)?;
    let metadata = json!({ "mode": capture.mode });
    ok_json_with_image(metadata, &capture)
}

fn edit_file(args: &Value, state: &AppState) -> Result<Value> {
    let path = path_arg(args, state)?;
    let old = args.get("old_string").and_then(Value::as_str).unwrap_or("");
    let new = args.get("new_string").and_then(Value::as_str).unwrap_or("");
    let data = read_text_file_bounded(&path)?;
    let count = data.matches(old).count();
    if old.is_empty() || count != 1 {
        return Ok(error_result(format!("old_string matched {count} times")));
    }
    let updated = data.replacen(old, new, 1);
    fs::write(&path, updated)?;
    Ok(ok_json(json!({ "path": path, "replacements": 1 })))
}

fn web_fetch(args: &Value) -> Result<Value> {
    let url = args.get("url").and_then(Value::as_str).unwrap_or("");
    if let Err(err) = block_private_urls(url) {
        return Ok(error_result(err.to_string()));
    }
    let max_chars = args
        .get("max_chars")
        .and_then(Value::as_u64)
        .unwrap_or(20000) as usize;

    let client = reqwest::blocking::Client::new();
    let response = match client.get(url).send() {
        Ok(res) => res,
        Err(e) => return Ok(error_result(format!("Failed to fetch URL: {}", e))),
    };

    if !response.status().is_success() {
        return Ok(error_result(format!(
            "HTTP request failed with status: {}",
            response.status()
        )));
    }

    let mut text = match response.text() {
        Ok(t) => t,
        Err(e) => return Ok(error_result(format!("Failed to read response body: {}", e))),
    };

    if text.len() > max_chars {
        text.truncate(max_chars);
    }
    Ok(ok_text(text))
}

fn http_request(args: &Value) -> Result<Value> {
    let method_str = args.get("method").and_then(Value::as_str).unwrap_or("GET");
    let url_str = args.get("url").and_then(Value::as_str).unwrap_or("");
    if let Err(err) = block_private_urls(url_str) {
        return Ok(error_result(err.to_string()));
    }

    let method = match method_str.to_uppercase().as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        "PATCH" => reqwest::Method::PATCH,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        other => return Ok(error_result(format!("unsupported HTTP method: {other}"))),
    };

    let client = reqwest::blocking::Client::new();
    let mut request = client.request(method, url_str);

    if let Some(headers) = args.get("headers").and_then(Value::as_object) {
        for (name, value) in headers {
            let val_str = value
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| value.to_string());
            request = request.header(name, val_str);
        }
    }

    if let Some(body) = args.get("body").and_then(Value::as_str) {
        request = request.body(body.to_string());
    }

    match request.send() {
        Ok(response) => {
            let status = response.status().as_u16();
            let success = response.status().is_success();
            let body = response.text().unwrap_or_default();

            Ok(ok_json(json!({
                "success": success,
                "status": status,
                "body": body,
                "stderr": ""
            })))
        }
        Err(e) => Ok(ok_json(json!({
            "success": false,
            "status": 1,
            "body": "",
            "stderr": e.to_string()
        }))),
    }
}

fn delete_file(args: &Value, state: &AppState) -> Result<Value> {
    let path = path_arg(args, state)?;
    if args.get("recursive").and_then(Value::as_bool) == Some(true) {
        return Ok(error_result(
            "recursive deletion unavailable: between-effect cancellation is not supported",
        ));
    }
    let metadata = fs::metadata(&path)?;
    if metadata.is_dir() {
        fs::remove_dir(&path)?;
    } else {
        fs::remove_file(&path)?;
    }
    Ok(ok_json(json!({ "path": path, "deleted": true })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::AppState;
    use crate::policy::PermissionMode;
    use serial_test::serial;

    #[test]
    fn image_arguments_reject_paths_endpoints_and_interactive_scopes() {
        for valid in [json!({}), json!({ "retina": false })] {
            assert!(valid_capture_args(&valid));
        }
        for invalid in [
            json!({ "path": "/tmp/caller-selected.png" }),
            json!({ "endpoint": "http://127.0.0.1:9222" }),
            json!({ "mode": "display" }),
            json!({ "mode": "screen", "app": "Browser" }),
            json!({ "app": "x".repeat(257) }),
        ] {
            assert!(!valid_capture_args(&invalid));
        }
    }

    #[test]
    #[serial]
    fn private_capture_path_is_restricted_and_removed_by_its_guard() {
        let directory = tempfile::TempDir::new().unwrap();
        let original = std::env::var_os("XDG_CACHE_HOME");
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", directory.path());
        }
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        let path = private_capture_path(&state).unwrap();
        let guard = CapturePathGuard(path.clone());
        assert!(path.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        drop(guard);
        assert!(!path.exists());
        unsafe {
            if let Some(original) = original {
                std::env::set_var("XDG_CACHE_HOME", original);
            } else {
                std::env::remove_var("XDG_CACHE_HOME");
            }
        }
    }

    #[test]
    fn expired_image_artifact_is_removed_without_another_capture() {
        let directory = tempfile::TempDir::new().unwrap();
        let cache = directory.path().join("artifacts");
        fs::create_dir(&cache).unwrap();
        fs::write(cache.join("artifacts.lock"), []).unwrap();
        let path = cache.join("0".repeat(64));
        fs::write(&path, [1_u8, 2, 3, 4]).unwrap();
        let modified = SystemTime::now()
            .checked_sub(MAX_ARTIFACT_AGE + Duration::from_secs(1))
            .unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();

        expire_artifact(&path).unwrap();

        assert!(!path.exists());
    }

    #[test]
    #[serial]
    fn image_artifact_cache_is_private_bounded_and_content_addressed() {
        let directory = tempfile::TempDir::new().unwrap();
        let original = std::env::var_os("XDG_CACHE_HOME");
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", directory.path());
        }
        let result = std::panic::catch_unwind(|| {
            let state = AppState::new(PermissionMode::Full, false).unwrap();
            let source = directory.path().join("capture.png");
            fs::write(&source, [1_u8, 2, 3, 4]).unwrap();
            let capture = ImageCapture {
                path: source.clone(),
                mode: ImageMode::Screen,
                bytes: 4,
                mime_type: "image/png".to_string(),
                ephemeral: false,
            };
            let response =
                cache_image_artifact(json!({ "mode": "screen" }), &capture, &state).unwrap();
            let artifact = &response["structuredContent"]["artifact"];
            let digest = artifact["sha256"].as_str().unwrap();
            let cached = directory
                .path()
                .join("poke-around")
                .join("artifacts")
                .join(digest);

            assert_eq!(artifact["locator"], format!("sha256:{digest}"));
            assert_eq!(artifact["bytes"], 4);
            assert_eq!(fs::read(&cached).unwrap(), [1_u8, 2, 3, 4]);
            assert!(
                !response
                    .to_string()
                    .contains(&source.to_string_lossy().to_string())
            );
            assert!(!response.to_string().contains("AQIDBA=="));
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    fs::metadata(&cached).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
            for index in 0..=MAX_ARTIFACT_COUNT {
                fs::write(&source, index.to_be_bytes()).unwrap();
                cache_image_artifact(json!({ "mode": "screen" }), &capture, &state).unwrap();
            }
            let count = fs::read_dir(cached.parent().unwrap())
                .unwrap()
                .filter_map(std::result::Result::ok)
                .filter(|entry| {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    name.len() == 64
                })
                .count();
            assert_eq!(count, MAX_ARTIFACT_COUNT);
        });
        unsafe {
            if let Some(original) = original {
                std::env::set_var("XDG_CACHE_HOME", original);
            } else {
                std::env::remove_var("XDG_CACHE_HOME");
            }
        }
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }

    #[test]
    fn read_image_should_return_only_an_external_hash_locator() {
        let path =
            std::env::temp_dir().join(format!("poke-around-read-image-{}.png", std::process::id()));
        fs::write(&path, [1_u8, 2, 3, 4]).unwrap();
        let state = AppState::new(PermissionMode::Full, false).unwrap();

        let response = read_image(&json!({ "path": path }), &state).unwrap();
        let content = response["content"].as_array().unwrap();

        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(response["structuredContent"]["artifact"]["bytes"], 4);
        assert!(
            response["structuredContent"]["artifact"]["locator"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(!response.to_string().contains("AQIDBA=="));
        assert!(
            !response
                .to_string()
                .contains(&path.to_string_lossy().to_string())
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn execute_tool_unknown_tool_should_return_error() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        let response = execute_tool("non_existent_tool", &json!({}), &state).unwrap();

        assert_eq!(response["isError"], true);
        let content = response["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "Unknown tool: non_existent_tool");
    }

    #[test]
    fn execute_tool_should_propagate_io_errors_from_tool_execution() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        let args = json!({ "path": "~/poke_around_test_nonexistent_file_12345" });

        let response = execute_tool("read_file", &args, &state);

        assert!(response.is_err());
        match response.unwrap_err() {
            crate::Error::Io(_) => {}
            other => panic!("Expected IO error, got {other:?}"),
        }
    }

    #[test]
    fn read_handlers_enforce_resource_limits() {
        let directory = tempfile::TempDir::new().unwrap();
        let oversized = directory.path().join("oversized.txt");
        File::create(&oversized)
            .unwrap()
            .set_len(MAX_FILE_READ_BYTES + 1)
            .unwrap();
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        assert!(read_file(&json!({ "path": oversized }), &state).is_err());

        for index in 0..=MAX_DIRECTORY_ENTRIES {
            File::create(directory.path().join(index.to_string())).unwrap();
        }
        assert!(list_directory(&json!({ "path": directory.path() }), &state).is_err());
    }

    #[test]
    fn targeted_effects_are_unavailable() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        for tool in [
            "run_command",
            "click",
            "press",
            "type",
            "paste",
            "hotkey",
            "scroll",
            "move",
            "set_value",
            "perform_action",
            "window",
            "app",
            "menu",
            "run",
            "clean",
        ] {
            let response = execute_tool(tool, &json!({}), &state).unwrap();
            assert_eq!(response["isError"], true);
        }
        for tool in ["swipe", "drag"] {
            let response = execute_tool(tool, &json!({}), &state).unwrap();
            assert_eq!(response["isError"], true);
        }
    }

    #[test]
    fn cancellation_before_effect_does_not_write() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        let path = std::env::temp_dir().join(format!(
            "poke-around-cancelled-write-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        let result = execute_tool_with_control(
            "write_file",
            &json!({ "path": path, "content": "not written" }),
            &state,
            &cancellation,
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();

        assert_eq!(
            result["structuredContent"]["status"],
            "CANCELLED_BEFORE_EFFECT"
        );
        assert_eq!(result["structuredContent"]["retry_safe"], true);
        assert!(!path.exists());
    }

    #[test]
    fn recursive_deletion_fails_before_effect() {
        let directory = tempfile::TempDir::new().unwrap();
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("file"), "value").unwrap();
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        let result = delete_file(
            &json!({ "path": nested.clone(), "recursive": true }),
            &state,
        )
        .unwrap();

        assert_eq!(result["isError"], true);
        assert!(nested.exists());
    }

    #[test]
    fn shell_execution_fails_closed() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        let result = execute_tool_with_control(
            "run_command",
            &json!({ "command": "printf poke" }),
            &state,
            &CancellationToken::default(),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(result["isError"], true);
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("unavailable")
        );
    }

    #[test]
    fn approval_summaries_are_informed_without_exposing_secrets() {
        let secret = "TOP_SECRET_CREDENTIAL";
        let command = tool_summary(
            "run_command",
            &json!({ "command": format!("API_KEY={secret} curl --token {secret}") }),
        );
        let http = tool_summary(
            "http_request",
            &json!({
                "method": "POST",
                "url": format!("https://user:{secret}@example.com/private?token={secret}"),
                "headers": { "Authorization": secret },
                "body": secret
            }),
        );
        let file = tool_summary(
            "write_file",
            &json!({ "path": "/private/account/credentials.txt", "content": secret }),
        );
        let typed = tool_summary("type", &json!({ "text": secret, "app": "Notes" }));
        let pasted = tool_summary("paste", &json!({ "text": secret }));
        let clipboard = tool_summary("clipboard_write", &json!({ "text": secret }));

        assert!(command.contains("curl"));
        assert!(command.contains("sha256:"));
        assert_eq!(http, "HTTP POST to https://example.com");
        assert!(file.contains(&normalized_path_label("/private/account/credentials.txt")));
        assert!(typed.contains("21 chars in 'Notes'"));
        assert!(pasted.contains("21 chars"));
        for summary in [command, http, file, typed, pasted, clipboard] {
            assert!(!summary.contains(secret));
            assert!(!summary.contains("Authorization"));
        }
    }

    #[test]
    fn approval_summaries_distinguish_risky_subtypes_without_selectors() {
        let launch = tool_summary("app", &json!({ "action": "launch", "app": "Safari" }));
        let quit = tool_summary("app", &json!({ "action": "quit", "app": "Safari" }));
        let set_value = tool_summary(
            "set_value",
            &json!({ "on": "secret selector", "value": "private value" }),
        );
        let perform = tool_summary(
            "perform_action",
            &json!({ "on": "secret selector", "action": "press" }),
        );

        assert_ne!(launch, quit);
        assert_eq!(launch, "App 'launch' for 'Safari'");
        assert_eq!(quit, "App 'quit' for 'Safari'");
        assert_eq!(set_value, "Set element value to 13 chars");
        assert_eq!(perform, "Perform 'press' on element reference");
        assert!(!set_value.contains("secret selector"));
        assert!(!set_value.contains("private value"));
        assert!(!perform.contains("secret selector"));
    }
}
