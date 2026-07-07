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
        "rm -r",
        "rm -R",
        "rmdir ",
        "unlink ",
        "truncate ",
        "truncate\t",
        "dd ",
        "dd\t",
        "dd if=",
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
        "$(rm",
        "$( rm",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern))
        || has_subshell_or_backtick_bypass(command)
        || has_find_exec_or_xargs(&lower)
        || has_shell_pipe_bypass(&lower)
}

fn has_subshell_or_backtick_bypass(command: &str) -> bool {
    command.contains("$(") || command.contains('`')
}

fn has_find_exec_or_xargs(lower: &str) -> bool {
    lower.contains("-exec ")
        || lower.contains("-execdir ")
        || lower.contains(" xargs ")
        || lower.starts_with("xargs ")
        || lower.contains("\txargs ")
}

fn has_shell_pipe_bypass(lower: &str) -> bool {
    const SHELL_REDIRECTS: &[&str] = &[
        "| sh",
        "|sh",
        "| bash",
        "|bash",
        "| zsh",
        "|zsh",
        "| dash",
        "|dash",
        "|/bin/sh",
        "| /bin/sh",
        "|/bin/bash",
        "| /bin/bash",
        "|/bin/zsh",
        "| /bin/zsh",
        "|/bin/dash",
        "| /bin/dash",
        "| sh -c",
        "| bash -c",
        "| zsh -c",
    ];
    SHELL_REDIRECTS
        .iter()
        .any(|pattern| lower.contains(pattern))
}

pub fn split_command_segments(command: &str) -> impl Iterator<Item = &str> + '_ {
    let mut parts = vec![command];
    for delimiter in [";", "\n", "&&", "||"] {
        parts = parts
            .into_iter()
            .flat_map(|part| part.split(delimiter))
            .collect();
    }
    parts = parts
        .into_iter()
        .flat_map(|part| split_on_single_pipe(part).into_iter())
        .collect();
    parts
        .into_iter()
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

fn split_on_single_pipe(segment: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let bytes = segment.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'|' {
            if index + 1 < bytes.len() && bytes[index + 1] == b'|' {
                index += 2;
                continue;
            }
            parts.push(&segment[start..index]);
            start = index + 1;
        }
        index += 1;
    }
    parts.push(&segment[start..]);
    parts
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_executable_empty() {
        assert_eq!(extract_executable(""), "");
        assert_eq!(extract_executable("   "), "");
    }

    #[test]
    fn test_extract_executable_plain() {
        assert_eq!(extract_executable("ls"), "ls");
        assert_eq!(extract_executable("ls -la"), "ls");
        assert_eq!(extract_executable("  ls  -la  "), "ls");
    }

    #[test]
    fn test_extract_executable_absolute_path() {
        assert_eq!(extract_executable("/bin/ls"), "ls");
        assert_eq!(extract_executable("/usr/bin/python3 script.py"), "python3");
    }

    #[test]
    fn test_extract_executable_relative_path() {
        assert_eq!(extract_executable("./script.sh"), "script.sh");
        assert_eq!(extract_executable("../bin/run"), "run");
    }

    #[test]
    fn test_split_command_segments() {
        assert_eq!(
            split_command_segments("ls -l").collect::<Vec<_>>(),
            vec!["ls -l"]
        );
        assert_eq!(
            split_command_segments("ls -l | grep 'foo'").collect::<Vec<_>>(),
            vec!["ls -l", "grep 'foo'"]
        );
        assert_eq!(
            split_command_segments("build && test").collect::<Vec<_>>(),
            vec!["build", "test"]
        );
        assert_eq!(
            split_command_segments("cat file.txt || echo 'failed'").collect::<Vec<_>>(),
            vec!["cat file.txt", "echo 'failed'"]
        );
        assert_eq!(
            split_command_segments("cd dir; ls").collect::<Vec<_>>(),
            vec!["cd dir", "ls"]
        );
        assert_eq!(
            split_command_segments("echo 'line 1'\necho 'line 2'").collect::<Vec<_>>(),
            vec!["echo 'line 1'", "echo 'line 2'"]
        );
        assert_eq!(
            split_command_segments("ls -l | grep 'foo' && echo 'bar'; pwd").collect::<Vec<_>>(),
            vec!["ls -l", "grep 'foo'", "echo 'bar'", "pwd"]
        );
        assert!(split_command_segments("").collect::<Vec<_>>().is_empty());
    }
}
