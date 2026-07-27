//! Spawns the real backend binary in `--botan-shim` mode and drives the
//! botan surface ani-cli uses through actual pipes — the wrapper script
//! the backend provisions execs exactly this.
//!
//! A binary that does not recognize the flag would start the HTTP
//! server and block; the helper polls with a deadline and fails loudly
//! instead of hanging the suite.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_ani-gui-backend");

fn run_shim(args: &[&str], stdin_bytes: &[u8]) -> (i32, Vec<u8>) {
    let mut child = Command::new(BIN)
        .arg("--botan-shim")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn backend in shim mode");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(stdin_bytes)
        .expect("write stdin");

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => {
                let mut out = Vec::new();
                child
                    .stdout
                    .take()
                    .expect("piped stdout")
                    .read_to_end(&mut out)
                    .expect("read stdout");
                return (status.code().unwrap_or(-1), out);
            }
            None if Instant::now() > deadline => {
                let _ = child.kill();
                panic!("--botan-shim did not exit; the binary likely started the server instead");
            }
            None => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn version_reports_botan3_syntax() {
    let (code, out) = run_shim(&["--version"], b"");
    assert_eq!(code, 0);
    assert!(
        out.starts_with(b"3"),
        "first char selects ani-cli's v3 syntax, got: {}",
        String::from_utf8_lossy(&out)
    );
}

#[test]
fn hash_and_hex_dec_match_the_reference_pipeline() {
    let (code, out) = run_shim(&["hash", "--no-fsname"], b"abc");
    assert_eq!(code, 0);
    assert_eq!(
        out,
        b"BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD\n"
    );

    let (code, out) = run_shim(&["hex_dec", "-"], b"414243\n");
    assert_eq!(code, 0);
    assert_eq!(out, b"ABC");
}

#[test]
fn cipher_round_trips_through_the_binary() {
    let key = format!("--key={}", hex(&[11u8; 32]));
    let nonce = format!("--nonce={}", hex(&[13u8; 12]));

    let (code, blob) = run_shim(
        &["cipher", "--cipher=AES-256/GCM", &key, &nonce, "-"],
        b"through real pipes",
    );
    assert_eq!(code, 0);
    assert_eq!(blob.len(), b"through real pipes".len() + 16);

    let (code, back) = run_shim(
        &[
            "cipher",
            "--decrypt",
            "--cipher=AES-256/GCM",
            &key,
            &nonce,
            "-",
        ],
        &blob,
    );
    assert_eq!(code, 0);
    assert_eq!(back, b"through real pipes");
}

#[test]
fn auth_failure_exits_one() {
    let key = format!("--key={}", hex(&[11u8; 32]));
    let wrong = format!("--key={}", hex(&[12u8; 32]));
    let nonce = format!("--nonce={}", hex(&[13u8; 12]));
    let (code, blob) = run_shim(
        &["cipher", "--cipher=AES-256/GCM", &key, &nonce, "-"],
        b"secret",
    );
    assert_eq!(code, 0);
    let (code, out) = run_shim(
        &[
            "cipher",
            "--decrypt",
            "--cipher=AES-256/GCM",
            &wrong,
            &nonce,
            "-",
        ],
        &blob,
    );
    assert_eq!(code, 1);
    assert!(out.is_empty());
}
