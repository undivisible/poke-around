use serde_json::{Value, json};

pub const TOOL_NAMES: &[&str] = &[
    "run_command",
    "network_speed",
    "read_file",
    "write_file",
    "list_directory",
    "system_info",
    "read_image",
    "run_agent",
    "take_screenshot",
    "edit_file",
    "web_fetch",
    "http_request",
    "delete_file",
    "image",
    "see",
    "list_screens",
    "permissions",
    "click",
    "press",
    "type",
    "paste",
    "hotkey",
    "scroll",
    "swipe",
    "drag",
    "move",
    "set_value",
    "perform_action",
    "window",
    "app",
    "open",
    "menu",
    "clipboard_read",
    "clipboard_write",
    "run",
    "sleep",
    "clean",
];

pub fn tools_json() -> String {
    let mut tools = base_tools();
    tools.extend(computer_use_tools());
    serde_json::to_string(&tools).expect("tools json serializes")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tools_json_output() {
        let json_str = tools_json();
        let parsed: Value =
            serde_json::from_str(&json_str).expect("tools_json should return valid JSON");

        let tools_array = parsed
            .as_array()
            .expect("tools_json should return a JSON array");
        assert_eq!(tools_array.len(), TOOL_NAMES.len());

        let mut output_names = std::collections::HashSet::new();

        for tool in tools_array {
            let obj = tool.as_object().expect("each tool should be a JSON object");

            assert!(
                obj.contains_key("name"),
                "tool missing 'name' field: {:?}",
                obj
            );
            assert!(
                obj.contains_key("description"),
                "tool missing 'description' field: {:?}",
                obj
            );
            assert!(
                obj.contains_key("inputSchema"),
                "tool missing 'inputSchema' field: {:?}",
                obj
            );

            let name = obj["name"].as_str().expect("name should be a string");
            output_names.insert(name.to_string());
        }

        for &expected_name in TOOL_NAMES {
            assert!(
                output_names.contains(expected_name),
                "Expected tool name '{}' not found in tools_json output",
                expected_name
            );
        }
    }
}
