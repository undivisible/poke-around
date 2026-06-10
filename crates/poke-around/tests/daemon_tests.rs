use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

#[cfg(unix)]
#[test]
fn daemon_starts_and_stops_gracefully() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_poke-around"))
        .arg("daemon")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon");

    let stderr = child.stderr.take().unwrap();
    let mut reader = BufReader::new(stderr);
    let mut line = String::new();
    let mut started = false;

    // Wait up to ~5 seconds for the "listening on" message
    for _ in 0..50 {
        line.clear();
        if let Ok(bytes) = reader.read_line(&mut line) {
            if bytes > 0 && line.contains("listening on") {
                started = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    assert!(started, "Daemon did not start properly or time out");

    // Send SIGINT
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }

    // Wait for the process to exit
    let status = child.wait().expect("Failed to wait on child");

    use std::os::unix::process::ExitStatusExt;
    assert!(
        status.success() || status.signal() == Some(libc::SIGINT),
        "Daemon did not exit successfully, status: {:?}",
        status
    );
}
