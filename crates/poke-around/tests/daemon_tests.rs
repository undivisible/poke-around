#[cfg(unix)]
use std::io::{BufRead, BufReader};
#[cfg(unix)]
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
#[test]
fn daemon_starts_and_stops_gracefully() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_poke-around"))
        .arg("daemon")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon");

    let stderr = child.stderr.take().unwrap();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        let mut log = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    log.push_str(&line);
                    if line.contains("listening on") {
                        let _ = started_tx.send(true);
                    }
                }
                Err(_) => break,
            }
        }
        log
    });
    let started = started_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or(false);

    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }
    let status = child.wait().expect("Failed to wait on child");
    let log = reader.join().unwrap_or_default();
    assert!(
        started,
        "Daemon did not start properly or time out. stderr:\n{log}"
    );

    use std::os::unix::process::ExitStatusExt;
    assert!(
        status.success() || status.signal() == Some(libc::SIGINT),
        "Daemon did not exit successfully, status: {:?}. stderr:\n{log}",
        status
    );
}

#[cfg(unix)]
#[test]
fn per_action_mode_fails_closed_without_a_host_terminal() {
    let output = Command::new(env!("CARGO_BIN_EXE_poke-around"))
        .args(["daemon", "--approval-mode", "per-action"])
        .stdin(Stdio::null())
        .output()
        .expect("daemon should run");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("per-action approval mode requires an interactive host terminal")
    );
}
