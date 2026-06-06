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
    "git_operations",
    "delete_file",
];

pub fn tools_json() -> &'static str {
    r#"[
{"name":"run_command","description":"Execute a shell command on the user's machine and return stdout, stderr, and exit code.","inputSchema":{"type":"object","properties":{"command":{"type":"string"},"cwd":{"type":"string"},"approval_token":{"type":"string"},"approve":{"type":"boolean"},"remember_in_session":{"type":"boolean"},"remember_all_risky":{"type":"boolean"}},"required":["command"]}},
{"name":"network_speed","description":"Run a built-in internet speed test and return download/upload Mbps.","inputSchema":{"type":"object","properties":{"tests":{"type":"string","enum":["download","upload","both"]}}}},
{"name":"read_file","description":"Read the contents of a file on the user's machine.","inputSchema":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}},
{"name":"write_file","description":"Write content to a file on the user's machine.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"},"approval_token":{"type":"string"},"approve":{"type":"boolean"},"remember_all_risky":{"type":"boolean"}},"required":["path","content"]}},
{"name":"list_directory","description":"List files and directories at a given path with sizes and types.","inputSchema":{"type":"object","properties":{"path":{"type":"string"}}}},
{"name":"system_info","description":"Get system information: OS, hostname, architecture, uptime, memory.","inputSchema":{"type":"object","properties":{}}},
{"name":"read_image","description":"Read an image or binary file and return it as base64-encoded data.","inputSchema":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}},
{"name":"run_agent","description":"Run a Poke Around agent by name.","inputSchema":{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}},
{"name":"take_screenshot","description":"Take a screenshot of the user's screen.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"approval_token":{"type":"string"},"approve":{"type":"boolean"},"remember_all_risky":{"type":"boolean"}}}},
{"name":"edit_file","description":"Surgically replace an exact string in a file. Fails if old_string is not found or is ambiguous.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"old_string":{"type":"string"},"new_string":{"type":"string"},"approval_token":{"type":"string"},"approve":{"type":"boolean"},"remember_all_risky":{"type":"boolean"}},"required":["path","old_string","new_string"]}},
{"name":"web_fetch","description":"Fetch the text content of a URL and return it. Optionally truncate to max_chars.","inputSchema":{"type":"object","properties":{"url":{"type":"string"},"max_chars":{"type":"integer"}},"required":["url"]}},
{"name":"http_request","description":"Make an HTTP request with custom method, headers and body. Returns status code and response body.","inputSchema":{"type":"object","properties":{"method":{"type":"string","enum":["GET","POST","PUT","DELETE","PATCH","HEAD","OPTIONS"]},"url":{"type":"string"},"headers":{"type":"object"},"body":{"type":"string"}},"required":["method","url"]}},
{"name":"git_operations","description":"Run a git operation in the current directory or cwd.","inputSchema":{"type":"object","properties":{"operation":{"type":"string","enum":["status","diff","log","show","commit","add","checkout","branch","stash","reset","rev-parse"]},"args":{"type":"array","items":{"type":"string"}},"cwd":{"type":"string"},"approval_token":{"type":"string"},"approve":{"type":"boolean"},"remember_all_risky":{"type":"boolean"}},"required":["operation"]}},
{"name":"delete_file","description":"Delete a file or empty directory. Requires approval for non-empty directories.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"recursive":{"type":"boolean"},"approval_token":{"type":"string"},"approve":{"type":"boolean"},"remember_all_risky":{"type":"boolean"}},"required":["path"]}}
]"#
}
