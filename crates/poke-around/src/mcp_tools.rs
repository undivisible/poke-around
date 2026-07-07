use serde_json::{Value, json};
use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use crate::mcp::AppState;
use crate::mcp::{
    block_private_urls, canonicalize_path, ensure_path_allowed, error_result, expand_path,
    file_path_arg, int_arg, ok_json, ok_json_with_image, ok_text, optional_output_path, path_arg,
    query_target_from_args, str_arg, target_from_args,
};
use crate::{Error, Result};
use rs_peekaboo::automation::{Target, parse_point};
use rs_peekaboo::{Bounds, Direction, ImageCapture, ImageMode, Peekaboo, PeekabooConfig, Point};
use shlex::split as shlex_split;

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
    DestructiveOnly,
}

static TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "run_command",
        handler: |a, s| run_command(a, s),
        approval: ApprovalCategory::DestructiveOnly,
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
        approval: ApprovalCategory::None,
        summary: "Read image file",
    },
    ToolDef {
        name: "run_agent",
        handler: |a, _| run_agent(a),
        approval: ApprovalCategory::None,
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
        approval: ApprovalCategory::None,
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
        handler: |a, s| image(a, s),
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
        approval: ApprovalCategory::None,
        summary: "Check permissions",
    },
    ToolDef {
        name: "click",
        handler: |a, s| click(a, s),
        approval: ApprovalCategory::Always,
        summary: "Click on screen",
    },
    ToolDef {
        name: "press",
        handler: |a, s| press(a, s),
        approval: ApprovalCategory::Always,
        summary: "Press key",
    },
    ToolDef {
        name: "type",
        handler: |a, s| type_text(a, s),
        approval: ApprovalCategory::Always,
        summary: "Type text",
    },
    ToolDef {
        name: "paste",
        handler: |a, s| paste(a, s),
        approval: ApprovalCategory::Always,
        summary: "Paste text",
    },
    ToolDef {
        name: "hotkey",
        handler: |a, s| hotkey(a, s),
        approval: ApprovalCategory::Always,
        summary: "Press hotkey",
    },
    ToolDef {
        name: "scroll",
        handler: |a, s| scroll(a, s),
        approval: ApprovalCategory::Always,
        summary: "Scroll screen",
    },
    ToolDef {
        name: "swipe",
        handler: |a, s| swipe(a, s),
        approval: ApprovalCategory::Always,
        summary: "Swipe on screen",
    },
    ToolDef {
        name: "drag",
        handler: |a, s| drag(a, s),
        approval: ApprovalCategory::Always,
        summary: "Drag on screen",
    },
    ToolDef {
        name: "move",
        handler: |a, s| move_pointer(a, s),
        approval: ApprovalCategory::Always,
        summary: "Move pointer",
    },
    ToolDef {
        name: "set_value",
        handler: |a, s| set_value(a, s),
        approval: ApprovalCategory::Always,
        summary: "Set UI element value",
    },
    ToolDef {
        name: "perform_action",
        handler: |a, s| perform_action(a, s),
        approval: ApprovalCategory::Always,
        summary: "Perform UI action",
    },
    ToolDef {
        name: "window",
        handler: |a, s| window(a, s),
        approval: ApprovalCategory::Always,
        summary: "Manage window",
    },
    ToolDef {
        name: "app",
        handler: |a, s| app(a, s),
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
        handler: |a, s| menu(a, s),
        approval: ApprovalCategory::Always,
        summary: "Click menu item",
    },
    ToolDef {
        name: "clipboard_read",
        handler: |_, _| clipboard_read(),
        approval: ApprovalCategory::None,
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
        handler: |a, s| run_file(a, s),
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
        handler: |a, s| clean(a, s),
        approval: ApprovalCategory::Always,
        summary: "Clean snapshots",
    },
];

pub fn execute_tool(tool_name: &str, args: &Value, state: &AppState) -> Result<Value> {
    match find_tool(tool_name) {
        Some(tool) => (tool.handler)(args, state),
        None => Ok(error_result(format!("Unknown tool: {tool_name}"))),
    }
}

pub(crate) fn tool_approval(tool_name: &str) -> Option<ApprovalCategory> {
    find_tool(tool_name).map(|t| t.approval)
}

pub(crate) fn tool_summary(tool_name: &str, args: &Value) -> String {
    match tool_name {
        "run_command" => format!("Run command: {}", str_arg(args, "command").unwrap_or("")),
        "write_file" => format!("Write file: {}", str_arg(args, "path").unwrap_or("?")),
        "edit_file" => format!("Edit file: {}", str_arg(args, "path").unwrap_or("?")),
        "delete_file" => format!("Delete: {}", str_arg(args, "path").unwrap_or("?")),
        _ => find_tool(tool_name)
            .map(|t| t.summary)
            .unwrap_or("Take screenshot")
            .to_string(),
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
    tools
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
                        "approval_token": {
                            "type": "string",
                            "description": "Approval token returned by a previous AWAITING_APPROVAL response"
                        },
                        "approve": {
                            "type": "boolean",
                            "description": "Set true after user approves in chat"
                        },
                        "remember_in_session": {
                            "type": "boolean",
                            "description": "If true, remember this command for this session"
                        },
                        "remember_all_risky": {
                            "type": "boolean",
                            "description": "If true, auto-approve all risky tools for this session"
                        }
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
                        "approval_token": {
                            "type": "string",
                            "description": "Approval token returned by a previous AWAITING_APPROVAL response"
                        },
                        "approve": {
                            "type": "boolean",
                            "description": "Set true after user approves in chat"
                        },
                        "remember_all_risky": {
                            "type": "boolean",
                            "description": "If true, auto-approve all risky tools for this session"
                        }
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
                        "approval_token": {
                            "type": "string",
                            "description": "Approval token returned by a previous AWAITING_APPROVAL response"
                        },
                        "approve": {
                            "type": "boolean",
                            "description": "Set true after user approves in chat"
                        },
                        "remember_all_risky": {
                            "type": "boolean",
                            "description": "If true, auto-approve all risky tools for this session"
                        }
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
                        "approval_token": {
                            "type": "string",
                            "description": "Approval token returned by a previous AWAITING_APPROVAL response"
                        },
                        "approve": {
                            "type": "boolean",
                            "description": "Set true after user approves in chat"
                        },
                        "remember_all_risky": {
                            "type": "boolean",
                            "description": "If true, auto-approve all risky tools for this session"
                        }
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
                        "recursive": {
                            "type": "boolean",
                            "description": "Delete non-empty directories recursively"
                        },
                        "approval_token": {
                            "type": "string",
                            "description": "Approval token returned by a previous AWAITING_APPROVAL response"
                        },
                        "approve": {
                            "type": "boolean",
                            "description": "Set true after user approves in chat"
                        },
                        "remember_all_risky": {
                            "type": "boolean",
                            "description": "If true, auto-approve all risky tools for this session"
                        }
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
            "name": "image",
            "description": "Capture a screen or window image on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mode": {"type": "string", "description": "Capture mode: screen or window"},
                    "path": {"type": "string", "description": "Optional output path"},
                    "retina": {"type": "boolean", "description": "Capture at retina scale"}
                }
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
            "name": "click",
            "description": "Click a coordinate or resolved UI element on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "element_id": {"type": "string", "description": "Stable element ID from see"},
                    "snapshot": {"type": "string", "description": "Optional snapshot id from see"},
                    "x": {"type": "number", "description": "Screen x coordinate"},
                    "y": {"type": "number", "description": "Screen y coordinate"},
                    "button": {"type": "string", "description": "Mouse button: left or right"},
                    "count": {"type": "number", "description": "Click count"}
                }
            }
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
            "name": "swipe",
            "description": "Swipe between two coordinates on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from": {"type": "string", "description": "Start coordinate as x,y"},
                    "to": {"type": "string", "description": "End coordinate as x,y"},
                    "duration_ms": {"type": "number", "description": "Swipe duration in milliseconds"}
                },
                "required": ["from", "to"]
            }
        }),
        json!({
            "name": "drag",
            "description": "Drag between two coordinates on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from": {"type": "string", "description": "Start coordinate as x,y"},
                    "to": {"type": "string", "description": "End coordinate as x,y"},
                    "duration_ms": {"type": "number", "description": "Drag duration in milliseconds"}
                },
                "required": ["from", "to"]
            }
        }),
        json!({
            "name": "move",
            "description": "Move the pointer on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "element_id": {"type": "string", "description": "Stable element ID from see"},
                    "snapshot": {"type": "string", "description": "Optional snapshot id from see"},
                    "x": {"type": "number", "description": "Screen x coordinate"},
                    "y": {"type": "number", "description": "Screen y coordinate"}
                }
            }
        }),
        json!({
            "name": "set_value",
            "description": "Set the value of a resolved UI element on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "on": {"type": "string", "description": "Stable element ID or label from see"},
                    "value": {"type": "string", "description": "Value to set"},
                    "snapshot": {"type": "string", "description": "Optional snapshot id from see"}
                },
                "required": ["on", "value"]
            }
        }),
        json!({
            "name": "perform_action",
            "description": "Perform an accessibility action on a resolved UI element on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "on": {"type": "string", "description": "Stable element ID or label from see"},
                    "action": {"type": "string", "description": "Accessibility action to perform"},
                    "snapshot": {"type": "string", "description": "Optional snapshot id from see"}
                },
                "required": ["on", "action"]
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
            "name": "run",
            "description": "Execute a JSON automation script on the user's machine.",
            "inputSchema": {
                "type": "object",
                "properties": {"file": {"type": "string", "description": "Path to the script file"}},
                "required": ["file"]
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
fn image(args: &Value, _state: &AppState) -> Result<Value> {
    let mode = ImageMode::parse_or_err(str_arg(args, "mode").unwrap_or("screen"))?;
    let path = optional_output_path(args)?;
    let retina = args.get("retina").and_then(Value::as_bool).unwrap_or(true);
    let peekaboo = Peekaboo::with_config(PeekabooConfig::default());
    let capture = peekaboo.image(mode, path, retina)?;
    let metadata = json!({
        "path": capture.path,
        "mode": capture.mode,
        "bytes": capture.bytes,
        "mime_type": capture.mime_type,
        "ephemeral": capture.ephemeral,
    });
    ok_json_with_image(metadata, &capture)
}

fn see(args: &Value, _state: &AppState) -> Result<Value> {
    let app = str_arg(args, "app");
    let mode = ImageMode::parse_or_err(str_arg(args, "mode").unwrap_or("screen"))?;
    let path = optional_output_path(args)?;
    let retina = args.get("retina").and_then(Value::as_bool).unwrap_or(true);
    let peekaboo = Peekaboo::new();
    let capture = peekaboo.image(mode, path, retina)?;
    let snapshot_id = rs_peekaboo::cache::new_snapshot_id();
    let snapshot = rs_peekaboo::Snapshot {
        snapshot_id,
        elements: peekaboo.ui_elements(app)?,
    };
    rs_peekaboo::cache::save_snapshot(&snapshot)?;
    let metadata = json!({
        "snapshot_id": snapshot.snapshot_id,
        "elements": snapshot.elements,
        "image": {
            "path": capture.path,
            "mode": capture.mode,
            "bytes": capture.bytes,
            "mime_type": capture.mime_type,
            "ephemeral": capture.ephemeral,
        }
    });
    ok_json_with_image(metadata, &capture)
}

fn list_screens(_args: &Value) -> Result<Value> {
    Ok(ok_json(Peekaboo::new().list_screens()?))
}

fn permissions(args: &Value) -> Result<Value> {
    if str_arg(args, "action") == Some("grant") {
        Ok(ok_json(Peekaboo::new().grant_permissions()?))
    } else {
        Ok(ok_json(Peekaboo::new().permissions()))
    }
}

fn click(args: &Value, _state: &AppState) -> Result<Value> {
    let button = str_arg(args, "button").unwrap_or("left");
    let count = int_arg(args, "count").unwrap_or(1).max(1) as u32;
    Ok(ok_json(Peekaboo::new().click(
        target_from_args(args)?,
        button,
        count,
    )?))
}

fn press(args: &Value, _state: &AppState) -> Result<Value> {
    let key = str_arg(args, "key").unwrap_or("");
    let count = int_arg(args, "count").unwrap_or(1).max(1) as u32;
    let delay_ms = args.get("delay_ms").and_then(Value::as_u64);
    Ok(ok_json(Peekaboo::new().press(key, count, delay_ms)?))
}

fn type_text(args: &Value, _state: &AppState) -> Result<Value> {
    let text = str_arg(args, "text").unwrap_or("");
    let clear = args.get("clear").and_then(Value::as_bool).unwrap_or(false);
    let press_return = args.get("return").and_then(Value::as_bool).unwrap_or(false);
    let delay_ms = args.get("delay_ms").and_then(Value::as_u64);
    let app = str_arg(args, "app");
    Ok(ok_json(Peekaboo::new().type_text(
        text,
        clear,
        press_return,
        delay_ms,
        app,
    )?))
}

fn paste(args: &Value, _state: &AppState) -> Result<Value> {
    let text = str_arg(args, "text").unwrap_or("");
    Ok(ok_json(Peekaboo::new().paste(text)?))
}

fn hotkey(args: &Value, _state: &AppState) -> Result<Value> {
    let keys = str_arg(args, "keys").unwrap_or("");
    let parts = keys
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    Ok(ok_json(Peekaboo::new().hotkey(&parts)?))
}

fn scroll(args: &Value, _state: &AppState) -> Result<Value> {
    let dx = int_arg(args, "dx").unwrap_or(0);
    let dy = int_arg(args, "dy").unwrap_or(0);
    let (direction, amount) = if dx != 0 {
        let direction = if dx < 0 {
            Direction::Left
        } else {
            Direction::Right
        };
        (direction, dx.unsigned_abs().max(1) as u32)
    } else {
        let direction = if dy < 0 {
            Direction::Down
        } else {
            Direction::Up
        };
        (direction, dy.unsigned_abs().max(1) as u32)
    };
    Ok(ok_json(Peekaboo::new().scroll(direction, amount)?))
}

fn swipe(args: &Value, _state: &AppState) -> Result<Value> {
    let from = parse_point(str_arg(args, "from").unwrap_or("0,0")).unwrap_or(Point { x: 0, y: 0 });
    let to = parse_point(str_arg(args, "to").unwrap_or("0,0")).unwrap_or(Point { x: 0, y: 0 });
    let duration_ms = args
        .get("duration_ms")
        .and_then(Value::as_u64)
        .unwrap_or(250);
    Ok(ok_json(Peekaboo::new().swipe(
        Target::Point(from),
        Target::Point(to),
        duration_ms,
    )?))
}

fn drag(args: &Value, _state: &AppState) -> Result<Value> {
    let from = parse_point(str_arg(args, "from").unwrap_or("0,0")).unwrap_or(Point { x: 0, y: 0 });
    let to = parse_point(str_arg(args, "to").unwrap_or("0,0")).unwrap_or(Point { x: 0, y: 0 });
    let duration_ms = args
        .get("duration_ms")
        .and_then(Value::as_u64)
        .unwrap_or(250);
    Ok(ok_json(Peekaboo::new().drag(
        Target::Point(from),
        Target::Point(to),
        duration_ms,
    )?))
}

fn move_pointer(args: &Value, _state: &AppState) -> Result<Value> {
    Ok(ok_json(
        Peekaboo::new().move_cursor(target_from_args(args)?)?,
    ))
}

fn set_value(args: &Value, _state: &AppState) -> Result<Value> {
    let value = str_arg(args, "value").unwrap_or("");
    Ok(ok_json(
        Peekaboo::new().set_value(query_target_from_args(args)?, value)?,
    ))
}

fn perform_action(args: &Value, _state: &AppState) -> Result<Value> {
    let action = str_arg(args, "action").unwrap_or("");
    Ok(ok_json(
        Peekaboo::new().perform_action(query_target_from_args(args)?, action)?,
    ))
}

fn window(args: &Value, _state: &AppState) -> Result<Value> {
    let action = str_arg(args, "action").unwrap_or("");
    let bounds = match action {
        "move" => Some(Bounds {
            x: int_arg(args, "x").unwrap_or(0),
            y: int_arg(args, "y").unwrap_or(0),
            width: 0,
            height: 0,
        }),
        "resize" => Some(Bounds {
            x: 0,
            y: 0,
            width: int_arg(args, "width").unwrap_or(0),
            height: int_arg(args, "height").unwrap_or(0),
        }),
        "set-bounds" => Some(Bounds {
            x: int_arg(args, "x").unwrap_or(0),
            y: int_arg(args, "y").unwrap_or(0),
            width: int_arg(args, "width").unwrap_or(0),
            height: int_arg(args, "height").unwrap_or(0),
        }),
        _ => None,
    };
    Ok(ok_json(Peekaboo::new().window(
        action,
        str_arg(args, "app"),
        str_arg(args, "title"),
        bounds,
    )?))
}

fn app(args: &Value, _state: &AppState) -> Result<Value> {
    let action = str_arg(args, "action").unwrap_or("");
    if action == "list" {
        return Ok(ok_json(Peekaboo::new().app("list", None)?));
    }
    let app = str_arg(args, "app").unwrap_or("");
    Ok(ok_json(Peekaboo::new().app(action, Some(app))?))
}

fn open_target(args: &Value, _state: &AppState) -> Result<Value> {
    let target = str_arg(args, "target").unwrap_or("");
    let app = str_arg(args, "app");
    let no_focus = args
        .get("no_focus")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(ok_json(Peekaboo::new().open(target, app, no_focus)?))
}

fn menu(args: &Value, _state: &AppState) -> Result<Value> {
    let action = str_arg(args, "action").unwrap_or("");
    let app = str_arg(args, "app").unwrap_or("");
    if matches!(action, "list" | "list-all" | "inspect") {
        let action = if action == "inspect" { "list" } else { action };
        return Ok(ok_json(Peekaboo::new().menu(action, app, None, None)?));
    }
    let menu = str_arg(args, "menu").unwrap_or("");
    let item = str_arg(args, "item").unwrap_or("");
    Ok(ok_json(Peekaboo::new().menu(
        "click",
        app,
        Some(menu),
        Some(item),
    )?))
}

fn clipboard_read() -> Result<Value> {
    let text = Peekaboo::new().clipboard_read()?;
    Ok(ok_json(json!({ "text": text })))
}

fn clipboard_write(args: &Value) -> Result<Value> {
    let text = str_arg(args, "text").unwrap_or("");
    Ok(ok_json(Peekaboo::new().clipboard_write(text)?))
}

fn run_file(args: &Value, state: &AppState) -> Result<Value> {
    let path = file_path_arg(args, state)?;
    let results = Peekaboo::new().run_file(&path)?;
    Ok(ok_json(json!(results)))
}

fn sleep_cmd(args: &Value) -> Result<Value> {
    let seconds = args.get("seconds").and_then(Value::as_f64).unwrap_or(0.0);
    let millis = (seconds.max(0.0) * 1000.0) as u64;
    std::thread::sleep(Duration::from_millis(millis));
    Ok(ok_json(json!({ "slept_ms": millis })))
}

fn clean(args: &Value, _state: &AppState) -> Result<Value> {
    let all = args
        .get("all_snapshots")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let snapshot = str_arg(args, "snapshot");
    let removed = rs_peekaboo::cache::clean_snapshots(all, snapshot)?;
    Ok(ok_json(json!({ "removed": removed })))
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

fn run_command(args: &Value, state: &AppState) -> Result<Value> {
    let command = args.get("command").and_then(Value::as_str).unwrap_or("");
    let cwd = if let Some(path) = args.get("cwd").and_then(Value::as_str) {
        let expanded = expand_path(path, &state.inner.home);
        let canonical = canonicalize_path(&expanded)?;
        ensure_path_allowed(&canonical, state)?;
        canonical
    } else {
        state.inner.home.clone()
    };
    #[cfg(target_os = "windows")]
    let parsed_args =
        windows_split(command).ok_or_else(|| Error::msg("failed to parse command string"))?;
    #[cfg(not(target_os = "windows"))]
    let parsed_args =
        shlex_split(command).ok_or_else(|| Error::msg("failed to parse command string"))?;
    if parsed_args.is_empty() {
        return Err(Error::msg("command is empty after parsing"));
    }
    let output = Command::new(&parsed_args[0])
        .args(&parsed_args[1..])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()?;
    Ok(ok_json(json!({
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
        "exit_code": output.status.code().unwrap_or(1),
        "success": output.status.success()
    })))
}

#[cfg(target_os = "windows")]
fn windows_split(s: &str) -> Option<Vec<String>> {
    let mut args = Vec::new();
    let mut current_arg = String::new();
    let mut in_double_quotes = false;
    let mut in_single_quotes = false;
    let mut escape_next = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if escape_next {
            current_arg.push(c);
            escape_next = false;
            continue;
        }
        match c {
            '\\' => {
                if let Some(&next_c) = chars.peek() {
                    if next_c == '"' || next_c == '\'' {
                        escape_next = true;
                        continue;
                    }
                }
                current_arg.push(c);
            }
            '"' if !in_single_quotes => in_double_quotes = !in_double_quotes,
            '\'' if !in_double_quotes => in_single_quotes = !in_single_quotes,
            ' ' | '\t' | '\n' | '\r' if !in_double_quotes && !in_single_quotes => {
                if !current_arg.is_empty() {
                    args.push(current_arg.clone());
                    current_arg.clear();
                }
            }
            _ => current_arg.push(c),
        }
    }
    if !current_arg.is_empty() {
        args.push(current_arg);
    }
    Some(args)
}

fn read_file(args: &Value, state: &AppState) -> Result<Value> {
    Ok(ok_text(fs::read_to_string(path_arg(args, state)?)?))
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
    for entry in fs::read_dir(&path)? {
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
    let metadata = json!({ "path": path, "mime_type": mime });
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
    let capture = rs_peekaboo::Peekaboo::new().image(mode, path, true)?;
    let metadata = json!({
        "path": capture.path,
        "bytes": capture.bytes,
        "mime_type": capture.mime_type,
        "ephemeral": capture.ephemeral,
    });
    ok_json_with_image(metadata, &capture)
}

fn edit_file(args: &Value, state: &AppState) -> Result<Value> {
    let path = path_arg(args, state)?;
    let old = args.get("old_string").and_then(Value::as_str).unwrap_or("");
    let new = args.get("new_string").and_then(Value::as_str).unwrap_or("");
    let data = fs::read_to_string(&path)?;
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
    let recursive = args
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let metadata = fs::metadata(&path)?;
    if metadata.is_dir() {
        if recursive {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_dir(&path)?;
        }
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

    fn expected_image_error(err: &str) -> bool {
        err.contains("CommandFailed")
            || err.contains("no screenshot tool found")
            || err.contains("X server")
            || (cfg!(windows) && !err.is_empty())
    }

    #[test]
    fn image_should_return_mcp_image_content() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();

        let response = image(&json!({}), &state);
        if let Ok(response) = response {
            let content = response["content"].as_array().unwrap();

            assert_eq!(content.len(), 2);
            assert_eq!(content[0]["type"], "text");
            assert_eq!(content[1]["type"], "image");
            assert_eq!(content[1]["mimeType"], "image/png");

            let structured_content = &response["structuredContent"];
            assert_eq!(structured_content["mode"], "screen");
            assert_eq!(structured_content["ephemeral"], true);
            assert!(structured_content["path"].as_str().is_some());

            let path = std::path::PathBuf::from(structured_content["path"].as_str().unwrap());
            let _ = fs::remove_file(path);
        } else {
            let err = response.unwrap_err();
            assert!(expected_image_error(&err.to_string()));
        }
    }

    #[test]
    fn image_with_args_should_return_mcp_image_content() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();

        let path =
            std::env::temp_dir().join(format!("poke-around-test-image-{}.png", std::process::id()));
        let response = image(
            &json!({ "mode": "screen", "path": path, "retina": false }),
            &state,
        );

        if let Ok(response) = response {
            let content = response["content"].as_array().unwrap();

            assert_eq!(content.len(), 2);
            assert_eq!(content[0]["type"], "text");
            assert_eq!(content[1]["type"], "image");
            assert_eq!(content[1]["mimeType"], "image/png");

            let structured_content = &response["structuredContent"];
            assert_eq!(structured_content["mode"], "screen");
            assert_eq!(structured_content["ephemeral"], false);
            let expected = path.canonicalize().unwrap_or_else(|_| path.clone());
            let actual = std::path::PathBuf::from(structured_content["path"].as_str().unwrap());
            let actual = actual.canonicalize().unwrap_or(actual);
            assert_eq!(actual, expected);

            let _ = fs::remove_file(path);
        } else {
            let err = response.unwrap_err();
            assert!(expected_image_error(&err.to_string()));
        }
    }

    #[test]
    fn read_image_should_return_mcp_image_content() {
        let path =
            std::env::temp_dir().join(format!("poke-around-read-image-{}.png", std::process::id()));
        fs::write(&path, [1_u8, 2, 3, 4]).unwrap();
        let state = AppState::new(PermissionMode::Full, false).unwrap();

        let response = read_image(&json!({ "path": path }), &state).unwrap();
        let content = response["content"].as_array().unwrap();

        assert_eq!(content[1]["type"], "image");
        assert_eq!(content[1]["mimeType"], "image/png");
        assert_eq!(content[1]["data"], "AQIDBA==");
        assert!(response["structuredContent"].get("base64").is_none());

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
}
