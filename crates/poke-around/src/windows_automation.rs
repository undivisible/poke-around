use crate::{Error, Result};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

pub use rs_peekaboo::Point;
use rs_peekaboo::{Bounds, ImageCapture, ImageMode, UiElement};

pub fn image(mode: ImageMode, path: Option<PathBuf>) -> Result<ImageCapture> {
    let input = json!({
        "mode": mode,
        "path": path,
    });
    let value = run_json(IMAGE_SCRIPT, &input)?;
    Ok(ImageCapture {
        path: PathBuf::from(required_str(&value, "path")?),
        mode,
        bytes: value.get("bytes").and_then(Value::as_u64).unwrap_or(0),
        mime_type: required_str(&value, "mime_type")?.to_string(),
    })
}

pub fn ui_elements(app: Option<&str>) -> Result<Vec<UiElement>> {
    let value = run_json(UI_ELEMENTS_SCRIPT, &json!({ "app": app }))?;
    Ok(serde_json::from_value(value)?)
}

pub fn list_screens() -> Result<Value> {
    run_json(LIST_SCREENS_SCRIPT, &json!({}))
}

pub fn permissions() -> Value {
    json!({
        "screen_recording": true,
        "accessibility": true,
        "clipboard": true,
        "platform": "windows"
    })
}

pub fn click(point: Point, button: &str, count: u32) -> Result<Value> {
    run_json(
        MOUSE_SCRIPT,
        &json!({
            "action": "click",
            "x": point.x,
            "y": point.y,
            "button": button,
            "count": count.max(1),
        }),
    )
}

pub fn move_cursor(point: Point) -> Result<Value> {
    run_json(
        MOUSE_SCRIPT,
        &json!({
            "action": "move",
            "x": point.x,
            "y": point.y,
        }),
    )
}

pub fn drag(from: Point, to: Point, duration_ms: u64) -> Result<Value> {
    run_json(
        MOUSE_SCRIPT,
        &json!({
            "action": "drag",
            "from_x": from.x,
            "from_y": from.y,
            "to_x": to.x,
            "to_y": to.y,
            "duration_ms": duration_ms,
        }),
    )
}

pub fn swipe(from: Point, to: Point, duration_ms: u64) -> Result<Value> {
    drag(from, to, duration_ms)
}

pub fn scroll(dx: i64, dy: i64) -> Result<Value> {
    run_json(
        MOUSE_SCRIPT,
        &json!({
            "action": "scroll",
            "dx": dx,
            "dy": dy,
        }),
    )
}

pub fn press(key: &str, count: u32, delay_ms: Option<u64>) -> Result<Value> {
    run_json(
        KEYBOARD_SCRIPT,
        &json!({
            "action": "press",
            "keys": send_keys_for_key(key),
            "count": count.max(1),
            "delay_ms": delay_ms.unwrap_or(0),
        }),
    )
}

pub fn hotkey(keys: &str) -> Result<Value> {
    run_json(
        KEYBOARD_SCRIPT,
        &json!({
            "action": "hotkey",
            "keys": send_keys_for_hotkey(keys),
        }),
    )
}

pub fn type_text(
    text: &str,
    clear: bool,
    press_return: bool,
    delay_ms: Option<u64>,
) -> Result<Value> {
    run_json(
        KEYBOARD_SCRIPT,
        &json!({
            "action": "type",
            "text": text,
            "clear": clear,
            "return": press_return,
            "delay_ms": delay_ms.unwrap_or(0),
        }),
    )
}

pub fn paste(text: &str) -> Result<Value> {
    clipboard_write(text)?;
    hotkey("ctrl+v")
}

pub fn set_value(point: Point, value: &str) -> Result<Value> {
    click(point, "left", 1)?;
    type_text(value, true, false, None)
}

pub fn perform_action(point: Point, action: &str) -> Result<Value> {
    match action {
        "right_click" | "right-click" => click(point, "right", 1),
        "double_click" | "double-click" | "open" | "press" => click(point, "left", 2),
        _ => click(point, "left", 1),
    }
}

pub fn window(
    action: &str,
    app: Option<&str>,
    title: Option<&str>,
    bounds: Option<Bounds>,
) -> Result<Value> {
    run_json(
        WINDOW_SCRIPT,
        &json!({
            "action": action,
            "app": app,
            "title": title,
            "bounds": bounds,
        }),
    )
}

pub fn app(action: &str, app: Option<&str>) -> Result<Value> {
    run_json(
        APP_SCRIPT,
        &json!({
            "action": action,
            "app": app,
        }),
    )
}

pub fn open(target: &str, app: Option<&str>, no_focus: bool) -> Result<Value> {
    run_json(
        OPEN_SCRIPT,
        &json!({
            "target": target,
            "app": app,
            "no_focus": no_focus,
        }),
    )
}

pub fn menu(action: &str, app: &str, menu: Option<&str>, item: Option<&str>) -> Result<Value> {
    run_json(
        MENU_SCRIPT,
        &json!({
            "action": action,
            "app": app,
            "menu": menu,
            "item": item,
        }),
    )
}

pub fn clipboard_read() -> Result<String> {
    let value = run_json(CLIPBOARD_SCRIPT, &json!({ "action": "read" }))?;
    Ok(value
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string())
}

pub fn clipboard_write(text: &str) -> Result<Value> {
    run_json(
        CLIPBOARD_SCRIPT,
        &json!({ "action": "write", "text": text }),
    )
}

pub fn run_file(path: PathBuf) -> Result<Vec<Value>> {
    let data = std::fs::read_to_string(path)?;
    let file: rs_peekaboo::RunFile = serde_json::from_str(&data)?;
    let mut results = Vec::new();
    for step in file.steps {
        results.push(match step.command.as_str() {
            "click" => {
                let point = point_from_args(&step.args, None)?;
                click(point, "left", 1)?
            }
            "move" => {
                let point = point_from_args(&step.args, None)?;
                move_cursor(point)?
            }
            "type" => {
                let text = step.args.get("text").and_then(Value::as_str).unwrap_or("");
                type_text(text, false, false, None)?
            }
            "press" => {
                let key = step.args.get("key").and_then(Value::as_str).unwrap_or("");
                press(key, 1, None)?
            }
            "sleep" => {
                let seconds = step
                    .args
                    .get("seconds")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                std::thread::sleep(Duration::from_millis((seconds.max(0.0) * 1000.0) as u64));
                json!({ "slept_ms": (seconds.max(0.0) * 1000.0) as u64 })
            }
            other => json!({ "ok": false, "error": format!("unsupported run command: {other}") }),
        });
    }
    Ok(results)
}

pub fn point_from_args(args: &Value, snapshot: Option<&str>) -> Result<Point> {
    if let Some(element_id) = args
        .get("element_id")
        .or_else(|| args.get("on"))
        .and_then(Value::as_str)
    {
        return point_from_snapshot(element_id, snapshot);
    }
    Ok(Point {
        x: int_arg(args, "x").unwrap_or(0),
        y: int_arg(args, "y").unwrap_or(0),
    })
}

pub fn point_from_text(value: &str) -> Point {
    let (x, y) = value.split_once(',').unwrap_or(("0", "0"));
    Point {
        x: x.trim().parse().unwrap_or(0),
        y: y.trim().parse().unwrap_or(0),
    }
}

pub fn send_keys_for_hotkey(keys: &str) -> String {
    keys.split('+')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(|key| match key.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "win" | "windows" | "super" | "meta" | "ctrl" | "control" => {
                "^".to_string()
            }
            "shift" => "+".to_string(),
            "alt" | "option" => "%".to_string(),
            other => send_keys_for_key(other),
        })
        .collect::<String>()
}

pub fn send_keys_for_key(key: &str) -> String {
    match key.to_ascii_lowercase().as_str() {
        "enter" | "return" => "{ENTER}".to_string(),
        "escape" | "esc" => "{ESC}".to_string(),
        "tab" => "{TAB}".to_string(),
        "space" => " ".to_string(),
        "backspace" => "{BACKSPACE}".to_string(),
        "delete" | "del" => "{DELETE}".to_string(),
        "up" | "arrowup" => "{UP}".to_string(),
        "down" | "arrowdown" => "{DOWN}".to_string(),
        "left" | "arrowleft" => "{LEFT}".to_string(),
        "right" | "arrowright" => "{RIGHT}".to_string(),
        "home" => "{HOME}".to_string(),
        "end" => "{END}".to_string(),
        "pageup" => "{PGUP}".to_string(),
        "pagedown" => "{PGDN}".to_string(),
        f if function_key(f).is_some() => format!("{{{}}}", function_key(f).unwrap_or("")),
        other => other.to_string(),
    }
}

fn point_from_snapshot(query: &str, snapshot: Option<&str>) -> Result<Point> {
    let snapshot_id =
        snapshot.ok_or_else(|| Error::msg("snapshot is required for element targets"))?;
    let snapshot = rs_peekaboo::cache::load_snapshot(snapshot_id)?;
    let element = snapshot
        .elements
        .iter()
        .find(|element| {
            element.id == query
                || element.label.eq_ignore_ascii_case(query)
                || element
                    .label
                    .to_ascii_lowercase()
                    .contains(&query.to_ascii_lowercase())
        })
        .ok_or_else(|| Error::msg(format!("element not found in snapshot: {query}")))?;
    let bounds = element
        .bounds
        .as_ref()
        .ok_or_else(|| Error::msg(format!("element has no bounds: {query}")))?;
    Ok(bounds.center())
}

fn function_key(key: &str) -> Option<&'static str> {
    match key {
        "f1" => Some("F1"),
        "f2" => Some("F2"),
        "f3" => Some("F3"),
        "f4" => Some("F4"),
        "f5" => Some("F5"),
        "f6" => Some("F6"),
        "f7" => Some("F7"),
        "f8" => Some("F8"),
        "f9" => Some("F9"),
        "f10" => Some("F10"),
        "f11" => Some("F11"),
        "f12" => Some("F12"),
        _ => None,
    }
}

fn int_arg(args: &Value, key: &str) -> Option<i64> {
    args.get(key)?
        .as_i64()
        .or_else(|| args.get(key)?.as_f64().map(|n| n as i64))
}

fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::msg(format!("Windows automation response missing {key}")))
}

fn run_json(script: &str, input: &Value) -> Result<Value> {
    let output = Command::new(powershell_runtime())
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script)
        .arg(serde_json::to_string(input)?)
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(Error::msg(String::from_utf8_lossy(&output.stderr)));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(serde_json::from_str(text.trim())?)
}

fn powershell_runtime() -> &'static str {
    "powershell.exe"
}

const IMAGE_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$path = $inputObject.path
if (-not $path) {
  $path = Join-Path ([System.IO.Path]::GetTempPath()) ("poke-around-" + [System.Guid]::NewGuid().ToString() + ".png")
}
$parent = Split-Path -Parent $path
if ($parent) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
$bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
$bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($bounds.Left, $bounds.Top, 0, 0, $bitmap.Size)
$bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
[pscustomobject]@{ path = $path; mode = $inputObject.mode; bytes = (Get-Item $path).Length; mime_type = "image/png" } | ConvertTo-Json -Compress
"#;

const UI_ELEMENTS_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$condition = [System.Windows.Automation.Condition]::TrueCondition
$root = [System.Windows.Automation.AutomationElement]::RootElement
$collection = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $condition)
$items = New-Object System.Collections.Generic.List[object]
$index = 0
foreach ($element in $collection) {
  $name = $element.Current.Name
  if ($inputObject.app -and ($name -notlike ("*" + $inputObject.app + "*"))) { continue }
  $rect = $element.Current.BoundingRectangle
  $bounds = if ($rect.IsEmpty) { $null } else { [pscustomobject]@{ x = [int64]$rect.X; y = [int64]$rect.Y; width = [int64]$rect.Width; height = [int64]$rect.Height } }
  $items.Add([pscustomobject]@{ id = "win-$index"; role = $element.Current.ControlType.ProgrammaticName; label = $name; app = $name; window = $name; bounds = $bounds; state = [pscustomobject]@{ native_window_handle = $element.Current.NativeWindowHandle; enabled = $element.Current.IsEnabled } })
  $index += 1
}
$items | ConvertTo-Json -Compress -Depth 8
"#;

const LIST_SCREENS_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type -AssemblyName System.Windows.Forms
$screens = [System.Windows.Forms.Screen]::AllScreens | ForEach-Object {
  [pscustomobject]@{ name = $_.DeviceName; primary = $_.Primary; x = $_.Bounds.X; y = $_.Bounds.Y; width = $_.Bounds.Width; height = $_.Bounds.Height; scale_factor = 1 }
}
[pscustomobject]@{ screens = @($screens) } | ConvertTo-Json -Compress -Depth 4
"#;

const MOUSE_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class NativeMouse {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
}
"@
$LEFTDOWN = 0x0002
$LEFTUP = 0x0004
$RIGHTDOWN = 0x0008
$RIGHTUP = 0x0010
$WHEEL = 0x0800
if ($inputObject.action -eq "move") {
  [NativeMouse]::SetCursorPos([int]$inputObject.x, [int]$inputObject.y) | Out-Null
} elseif ($inputObject.action -eq "click") {
  [NativeMouse]::SetCursorPos([int]$inputObject.x, [int]$inputObject.y) | Out-Null
  $down = if ($inputObject.button -eq "right") { $RIGHTDOWN } else { $LEFTDOWN }
  $up = if ($inputObject.button -eq "right") { $RIGHTUP } else { $LEFTUP }
  for ($i = 0; $i -lt [int]$inputObject.count; $i++) {
    [NativeMouse]::mouse_event($down, 0, 0, 0, [UIntPtr]::Zero)
    [NativeMouse]::mouse_event($up, 0, 0, 0, [UIntPtr]::Zero)
  }
} elseif ($inputObject.action -eq "drag") {
  [NativeMouse]::SetCursorPos([int]$inputObject.from_x, [int]$inputObject.from_y) | Out-Null
  [NativeMouse]::mouse_event($LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds ([int]$inputObject.duration_ms)
  [NativeMouse]::SetCursorPos([int]$inputObject.to_x, [int]$inputObject.to_y) | Out-Null
  [NativeMouse]::mouse_event($LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
} elseif ($inputObject.action -eq "scroll") {
  $delta = if ([int]$inputObject.dy -ne 0) { -120 * [Math]::Sign([int]$inputObject.dy) } else { -120 * [Math]::Sign([int]$inputObject.dx) }
  [NativeMouse]::mouse_event($WHEEL, 0, 0, [uint32]$delta, [UIntPtr]::Zero)
}
[pscustomobject]@{ ok = $true; action = $inputObject.action } | ConvertTo-Json -Compress
"#;

const KEYBOARD_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type -AssemblyName System.Windows.Forms
if ($inputObject.action -eq "type") {
  if ($inputObject.clear) { [System.Windows.Forms.SendKeys]::SendWait("^a") }
  [System.Windows.Forms.Clipboard]::SetText([string]$inputObject.text)
  [System.Windows.Forms.SendKeys]::SendWait("^v")
  if ($inputObject.return) { [System.Windows.Forms.SendKeys]::SendWait("{ENTER}") }
} else {
  for ($i = 0; $i -lt [int]$inputObject.count; $i++) {
    [System.Windows.Forms.SendKeys]::SendWait([string]$inputObject.keys)
    if ([int]$inputObject.delay_ms -gt 0) { Start-Sleep -Milliseconds ([int]$inputObject.delay_ms) }
  }
}
[pscustomobject]@{ ok = $true; action = $inputObject.action } | ConvertTo-Json -Compress
"#;

const WINDOW_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class NativeWindow {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
}
"@
$windows = Get-Process | Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle }
if ($inputObject.action -eq "list") {
  $windows | ForEach-Object { [pscustomobject]@{ app = $_.ProcessName; title = $_.MainWindowTitle; pid = $_.Id; handle = $_.MainWindowHandle.ToInt64() } } | ConvertTo-Json -Compress
  exit
}
$target = $windows | Where-Object {
  (-not $inputObject.app -or $_.ProcessName -like ("*" + $inputObject.app + "*")) -and
  (-not $inputObject.title -or $_.MainWindowTitle -like ("*" + $inputObject.title + "*"))
} | Select-Object -First 1
if (-not $target) { throw "window not found" }
if ($inputObject.action -in @("focus", "activate", "switch")) {
  [NativeWindow]::SetForegroundWindow($target.MainWindowHandle) | Out-Null
} elseif ($inputObject.action -eq "minimize") {
  [NativeWindow]::ShowWindow($target.MainWindowHandle, 6) | Out-Null
} elseif ($inputObject.action -in @("restore", "unhide")) {
  [NativeWindow]::ShowWindow($target.MainWindowHandle, 9) | Out-Null
  [NativeWindow]::SetForegroundWindow($target.MainWindowHandle) | Out-Null
} elseif ($inputObject.action -eq "close") {
  [NativeWindow]::PostMessage($target.MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
} elseif ($inputObject.action -in @("move", "resize", "set-bounds")) {
  $b = $inputObject.bounds
  [NativeWindow]::SetWindowPos($target.MainWindowHandle, [IntPtr]::Zero, [int]$b.x, [int]$b.y, [int]$b.width, [int]$b.height, 0x0040) | Out-Null
}
[pscustomobject]@{ ok = $true; action = $inputObject.action; app = $target.ProcessName; title = $target.MainWindowTitle } | ConvertTo-Json -Compress
"#;

const APP_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
if ($inputObject.action -eq "list") {
  Get-Process | Where-Object { $_.MainWindowTitle } | ForEach-Object { [pscustomobject]@{ app = $_.ProcessName; title = $_.MainWindowTitle; pid = $_.Id } } | ConvertTo-Json -Compress
  exit
}
if ($inputObject.action -in @("launch", "open")) {
  Start-Process -FilePath ([string]$inputObject.app)
} else {
  $process = Get-Process | Where-Object { $_.ProcessName -like ("*" + $inputObject.app + "*") } | Select-Object -First 1
  if (-not $process) { throw "app not found" }
  if ($inputObject.action -in @("quit", "close")) {
    $process.CloseMainWindow() | Out-Null
  }
}
[pscustomobject]@{ ok = $true; action = $inputObject.action; app = $inputObject.app } | ConvertTo-Json -Compress
"#;

const OPEN_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
if ($inputObject.app) {
  Start-Process -FilePath ([string]$inputObject.app) -ArgumentList ([string]$inputObject.target)
} else {
  Start-Process -FilePath ([string]$inputObject.target)
}
[pscustomobject]@{ ok = $true; target = $inputObject.target } | ConvertTo-Json -Compress
"#;

const MENU_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
if ($inputObject.action -in @("list", "list-all", "inspect")) {
  [pscustomobject]@{ menus = @(); note = "Windows menu inspection is exposed through UI Automation snapshots." } | ConvertTo-Json -Compress
  exit
}
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.SendKeys]::SendWait("%")
[pscustomobject]@{ ok = $true; action = $inputObject.action; menu = $inputObject.menu; item = $inputObject.item } | ConvertTo-Json -Compress
"#;

const CLIPBOARD_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type -AssemblyName System.Windows.Forms
if ($inputObject.action -eq "write") {
  [System.Windows.Forms.Clipboard]::SetText([string]$inputObject.text)
  [pscustomobject]@{ ok = $true; bytes = ([string]$inputObject.text).Length } | ConvertTo-Json -Compress
} else {
  [pscustomobject]@{ text = [System.Windows.Forms.Clipboard]::GetText() } | ConvertTo-Json -Compress
}
"#;
