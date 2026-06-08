use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionMode {
    Full,
    Limited,
    Sandbox,
}

impl PermissionMode {
    pub fn parse(value: Option<&str>) -> Self {
        match value
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "limited" => Self::Limited,
            "sandbox" => Self::Sandbox,
            _ => Self::Full,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Limited => "limited",
            Self::Sandbox => "sandbox",
        }
    }
}

const SAFE_TOOLS: &[&str] = &[
    "read_file",
    "read_image",
    "list_directory",
    "system_info",
    "network_speed",
    "web_fetch",
    "http_request",
    "clipboard_read",
    "list_screens",
    "permissions",
];

const LIMITED_COMMANDS: &[&str] = &[
    "curl",
    "yt-dlp",
    "youtube-dl",
    "ls",
    "pwd",
    "cat",
    "grep",
    "find",
    "head",
    "tail",
    "wc",
    "sed",
    "awk",
    "which",
    "command",
    "echo",
    "stat",
    "du",
    "df",
    "ps",
    "uname",
    "sw_vers",
    "whoami",
    "jq",
    "diff",
];

const SANDBOX_COMMANDS: &[&str] = &[
    "yt-dlp",
    "youtube-dl",
    "ffmpeg",
    "ffprobe",
    "brew",
    "wax",
    "node",
    "bun",
    "python",
    "python3",
    "curl",
    "mktemp",
    "mkdir",
    "cp",
    "mv",
    "touch",
    "jq",
    "diff",
    "ls",
    "pwd",
    "cat",
    "grep",
    "find",
    "head",
    "tail",
    "wc",
    "sed",
    "awk",
    "which",
    "command",
    "echo",
    "stat",
    "du",
    "df",
    "ps",
    "uname",
    "sw_vers",
    "whoami",
];

pub fn evaluate_access_policy(
    tool_name: &str,
    args: &Value,
    mode: PermissionMode,
) -> Option<String> {
    if mode == PermissionMode::Full || SAFE_TOOLS.contains(&tool_name) {
        return None;
    }

    if tool_name == "run_command" {
        let command = args.get("command").and_then(Value::as_str).unwrap_or("");
        if command.trim().is_empty() {
            return Some("Command is empty.".to_string());
        }
        if has_dangerous_pattern(command) {
            return Some("Command matches a dangerous pattern.".to_string());
        }
        let allowlist = match mode {
            PermissionMode::Full => &[][..],
            PermissionMode::Limited => LIMITED_COMMANDS,
            PermissionMode::Sandbox => SANDBOX_COMMANDS,
        };
        for segment in split_command_segments(command) {
            let executable = extract_executable(segment);
            if executable.is_empty() || !allowlist.contains(&executable.as_str()) {
                return Some(format!(
                    "Command '{}' is not permitted in this mode.",
                    if executable.is_empty() {
                        "unknown"
                    } else {
                        executable.as_str()
                    }
                ));
            }
        }
        return None;
    }

    if matches!(
        tool_name,
        "write_file" | "take_screenshot" | "edit_file" | "delete_file"
    ) {
        return Some(format!(
            "Tool '{tool_name}' is disabled in {} mode.",
            mode.as_str()
        ));
    }

    if tool_name == "git_operations" {
        let operation = args.get("operation").and_then(Value::as_str).unwrap_or("");
        if matches!(
            operation,
            "status" | "diff" | "log" | "show" | "branch" | "rev-parse"
        ) {
            return None;
        }
        return Some(format!(
            "git operation '{operation}' is not permitted in {} mode.",
            mode.as_str()
        ));
    }

    Some(format!(
        "Tool '{tool_name}' is not permitted in {} mode.",
        mode.as_str()
    ))
}

pub fn is_destructive_command(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    [
        "rm ",
        "rm\t",
        "rmdir ",
        "unlink ",
        "mkfs",
        "diskutil erase",
        "> /",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern))
}

pub fn has_dangerous_pattern(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    [
        "sudo ",
        "rm -rf",
        "rm -fr",
        "rm -r -f",
        "diskutil erase",
        "mkfs.",
        "mkfs ",
        "shutdown",
        "reboot",
        "launchctl bootout",
        "chmod 777",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern))
        || lower.contains("| sh")
        || lower.contains("| bash")
        || lower.contains("| zsh")
}

pub fn split_command_segments(command: &str) -> impl Iterator<Item = &str> {
    command
        .split(&[';', '\n'][..])
        .flat_map(|part| part.split("&&"))
        .flat_map(|part| part.split("||"))
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

pub fn extract_executable(segment: &str) -> String {
    let segment = segment
        .trim_start_matches(|c: char| c == '(' || c.is_whitespace())
        .strip_prefix("sudo ")
        .unwrap_or(segment.trim());
    let raw = segment
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches(|c| c == '(' || c == ')');
    raw.rsplit('/').next().unwrap_or("").to_string()
}
