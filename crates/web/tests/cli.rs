//! Runs the real binary.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    process::{Child, Command, Stdio},
};

const BIN: &str = env!("CARGO_BIN_EXE_monopoly-deal-web");

#[test]
fn help_succeeds() {
    let out = Command::new(BIN).arg("--help").output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("usage:"));
}

#[test]
fn bad_option_fails() {
    let out = Command::new(BIN).arg("--nope").output().unwrap();
    assert!(!out.status.success());
}

fn get(port: u16, host: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        s,
        "GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    s.read_to_string(&mut response).unwrap();
    response
}

/// Kills the server even when an assertion fails.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn port_zero_reports_and_serves_the_bound_port() {
    let mut server = Server(
        Command::new(BIN)
            .args(["--port", "0", "--players", "2"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut lines = BufReader::new(server.0.stdout.take().unwrap()).lines();
    let first = lines.next().unwrap().unwrap();
    // Keep reading so the server never writes into a closed pipe.
    std::thread::spawn(move || lines.for_each(drop));
    let port: u16 = first
        .split("http://127.0.0.1:")
        .nth(1)
        .and_then(|rest| rest.split(' ').next())
        .and_then(|p| p.parse().ok())
        .unwrap_or_else(|| panic!("no address in {first:?}"));
    assert_ne!(port, 0);

    let ok = get(port, &format!("127.0.0.1:{port}"));
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
    let foreign = get(port, "evil.example");
    assert!(foreign.starts_with("HTTP/1.1 403"), "{foreign}");
}
