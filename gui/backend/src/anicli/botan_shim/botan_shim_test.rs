use super::*;

/// Test-side hex encoder so args can be built without production
/// helpers.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

// --- GCM primitives, pinned to the GCM spec's AES-256 test cases ---
// (values independently regenerated with python3-cryptography, the
// same implementation backing the bats harness's botan stand-in).

#[test]
fn gcm_encrypt_matches_the_empty_plaintext_nist_vector() {
    // Zero key, zero nonce, empty plaintext: output is the tag alone.
    let out = gcm_encrypt(&[0u8; 32], &[0u8; 12], b"").expect("encrypt");
    assert_eq!(hex(&out), "530f8afbc74536b9a963b4f1c4cb738b");
}

#[test]
fn gcm_encrypt_matches_the_sixteen_zero_byte_nist_vector() {
    let out = gcm_encrypt(&[0u8; 32], &[0u8; 12], &[0u8; 16]).expect("encrypt");
    assert_eq!(
        hex(&out),
        "cea7403d4d606b6e074ec5d3baf39d18d0d1c8a799996bf0265b98b5d48ab919"
    );
}

#[test]
fn gcm_roundtrip_recovers_the_plaintext() {
    let key: Vec<u8> = (0u8..32).collect();
    let nonce: Vec<u8> = (100u8..112).collect();
    let pt = b"the encrypted allanime transport";
    let blob = gcm_encrypt(&key, &nonce, pt).expect("encrypt");
    assert_eq!(blob.len(), pt.len() + 16, "tag is appended");
    let back = gcm_decrypt(&key, &nonce, &blob).expect("decrypt");
    assert_eq!(back, pt);
}

#[test]
fn gcm_decrypt_rejects_a_tampered_tag() {
    let key = [7u8; 32];
    let nonce = [9u8; 12];
    let mut blob = gcm_encrypt(&key, &nonce, b"payload").expect("encrypt");
    let last = blob.len() - 1;
    blob[last] ^= 0xff;
    assert!(gcm_decrypt(&key, &nonce, &blob).is_err());
}

#[test]
fn gcm_rejects_bad_key_and_nonce_lengths() {
    assert!(
        gcm_encrypt(&[0u8; 16], &[0u8; 12], b"x").is_err(),
        "short key"
    );
    assert!(
        gcm_encrypt(&[0u8; 32], &[0u8; 16], b"x").is_err(),
        "long nonce"
    );
    assert!(
        gcm_decrypt(&[0u8; 32], &[0u8; 12], &[0u8; 8]).is_err(),
        "blob shorter than a tag"
    );
}

// --- hash + hex ---

#[test]
fn sha256_matches_the_abc_vector_in_uppercase() {
    assert_eq!(
        sha256_hex_upper(b"abc"),
        "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
    );
}

#[test]
fn hex_decode_ignores_ascii_whitespace_and_accepts_both_cases() {
    assert_eq!(hex_decode("41 42\n43").expect("decode"), b"ABC");
    assert_eq!(hex_decode("DEADbeef\n").expect("decode"), unhex("deadbeef"));
}

#[test]
fn hex_decode_rejects_odd_length_and_bad_digits() {
    assert!(hex_decode("abc").is_err());
    assert!(hex_decode("zz").is_err());
}

// --- cipher argument parsing (the exact shapes ani-cli passes) ---

#[test]
fn parse_cipher_args_reads_the_encrypt_form() {
    let key = [3u8; 32];
    let nonce = [4u8; 12];
    let args: Vec<String> = [
        "--cipher=AES-256/GCM".into(),
        format!("--key={}", hex(&key)),
        format!("--nonce={}", hex(&nonce)),
        "-".into(),
    ]
    .into();
    let parsed = parse_cipher_args(&args).expect("parse");
    assert!(!parsed.decrypt);
    assert_eq!(parsed.key, key);
    assert_eq!(parsed.nonce, nonce);
}

#[test]
fn parse_cipher_args_reads_the_decrypt_form() {
    let args: Vec<String> = [
        "--decrypt".into(),
        "--cipher=AES-256/GCM".into(),
        format!("--key={}", hex(&[0u8; 32])),
        format!("--nonce={}", hex(&[0u8; 12])),
        "-".into(),
    ]
    .into();
    let parsed = parse_cipher_args(&args).expect("parse");
    assert!(parsed.decrypt);
}

#[test]
fn parse_cipher_args_requires_key_and_nonce() {
    let args: Vec<String> = vec!["--cipher=AES-256/GCM".into(), "-".into()];
    assert!(parse_cipher_args(&args).is_err());
}

// --- the dispatcher, end to end over in-memory streams ---

fn run(args: &[&str], stdin: &[u8]) -> (u8, Vec<u8>) {
    let args: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    let mut input = std::io::Cursor::new(stdin.to_vec());
    let mut output = Vec::new();
    let code = run_shim(&args, &mut input, &mut output);
    (code, output)
}

#[test]
fn run_shim_version_starts_with_three() {
    // ani-cli reads only the first character to pick the Botan-3
    // argument syntax.
    let (code, out) = run(&["--version"], b"");
    assert_eq!(code, 0);
    assert!(
        out.starts_with(b"3"),
        "got: {}",
        String::from_utf8_lossy(&out)
    );
}

#[test]
fn run_shim_hash_prints_uppercase_hex_with_newline() {
    let (code, out) = run(&["hash", "--no-fsname"], b"abc");
    assert_eq!(code, 0);
    assert_eq!(
        out,
        b"BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD\n"
    );
}

#[test]
fn run_shim_hex_dec_decodes_stdin_to_raw_bytes() {
    let (code, out) = run(&["hex_dec", "-"], b"414243\n");
    assert_eq!(code, 0);
    assert_eq!(out, b"ABC");
}

#[test]
fn run_shim_cipher_roundtrips_through_both_directions() {
    let key = hex(&[5u8; 32]);
    let nonce = hex(&[6u8; 12]);
    let enc_key = format!("--key={key}");
    let enc_nonce = format!("--nonce={nonce}");
    let (code, blob) = run(
        &["cipher", "--cipher=AES-256/GCM", &enc_key, &enc_nonce, "-"],
        b"plaintext payload",
    );
    assert_eq!(code, 0);
    assert_eq!(blob.len(), b"plaintext payload".len() + 16);

    let (code, back) = run(
        &[
            "cipher",
            "--decrypt",
            "--cipher=AES-256/GCM",
            &enc_key,
            &enc_nonce,
            "-",
        ],
        &blob,
    );
    assert_eq!(code, 0);
    assert_eq!(back, b"plaintext payload");
}

#[test]
fn run_shim_cipher_auth_failure_exits_one_with_empty_stdout() {
    let key = format!("--key={}", hex(&[5u8; 32]));
    let wrong_key = format!("--key={}", hex(&[9u8; 32]));
    let nonce = format!("--nonce={}", hex(&[6u8; 12]));
    let (code, blob) = run(
        &["cipher", "--cipher=AES-256/GCM", &key, &nonce, "-"],
        b"secret",
    );
    assert_eq!(code, 0);
    let (code, out) = run(
        &[
            "cipher",
            "--decrypt",
            "--cipher=AES-256/GCM",
            &wrong_key,
            &nonce,
            "-",
        ],
        &blob,
    );
    assert_eq!(code, 1);
    assert!(out.is_empty());
}

#[test]
fn run_shim_unknown_invocation_exits_two() {
    let (code, _) = run(&["keygen"], b"");
    assert_eq!(code, 2);
    let (code, _) = run(&[], b"");
    assert_eq!(code, 2);
}

// --- wrapper provisioning ---

#[test]
fn provision_writes_an_executable_wrapper_that_execs_the_shim() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let dir = tmp.path().join("botan-shim");
    let exe = std::path::Path::new("/opt/ani-gui/ani-gui-backend");

    let got = provision_botan_wrapper(&dir, exe).expect("provision");
    assert_eq!(got, dir, "returns the dir for PATH appending");

    let wrapper = dir.join("botan");
    let body = std::fs::read_to_string(&wrapper).expect("wrapper exists");
    assert!(body.starts_with("#!/bin/sh"), "sh shebang: {body}");
    assert!(
        body.contains("'/opt/ani-gui/ani-gui-backend' --botan-shim \"$@\""),
        "execs the backend shim (shell-safe single quotes) with args forwarded: {body}"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&wrapper)
            .expect("stat")
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "wrapper must be executable");
    }
}

#[test]
fn provision_is_idempotent_across_boots() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let dir = tmp.path().join("botan-shim");
    let exe = std::path::Path::new("/first/backend");
    provision_botan_wrapper(&dir, exe).expect("first provision");
    // A later boot from a different install path rewrites the wrapper.
    let exe2 = std::path::Path::new("/second/backend");
    provision_botan_wrapper(&dir, exe2).expect("second provision");
    let body = std::fs::read_to_string(dir.join("botan")).expect("wrapper");
    assert!(body.contains("/second/backend"), "self-heals: {body}");
}

#[test]
fn provision_own_botan_shim_returns_the_wrapper_dir() {
    let td = tempfile::tempdir().expect("tempdir");
    let got = provision_own_botan_shim(td.path()).expect("provisioned");
    assert_eq!(
        got,
        td.path()
            .join("botan-shim")
            .join(std::process::id().to_string())
    );
    assert!(got.join("botan").is_file(), "wrapper script written");
}

#[test]
fn provision_own_botan_shim_is_none_when_the_dir_cannot_be_created() {
    // A regular file where the cache root should be makes
    // create_dir_all fail; the helper must degrade to None, not error
    // out of AppState::build.
    let td = tempfile::tempdir().expect("tempdir");
    let file = td.path().join("not-a-dir");
    std::fs::write(&file, b"x").expect("write file");
    assert!(provision_own_botan_shim(&file).is_none());
}

#[cfg(unix)]
#[test]
fn provisioned_wrapper_survives_shell_metacharacters_in_the_exe_path() {
    // An install under a directory like `/opt/$channel v2/` must not
    // let /bin/sh expand or split the interpolated path — the wrapper
    // has to exec the literal executable.
    let td = tempfile::tempdir().expect("tempdir");
    let weird = td.path().join("$channel v2");
    std::fs::create_dir(&weird).expect("mkdir weird");
    let stub = weird.join("backend");
    std::fs::write(&stub, "#!/bin/sh\necho STUB-OK \"$@\"\n").expect("write stub");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    let dir = provision_botan_wrapper(&td.path().join("shim"), &stub).expect("provision");
    let out = std::process::Command::new("sh")
        .arg(dir.join("botan"))
        .arg("--version")
        .output()
        .expect("run wrapper");
    assert!(
        out.status.success(),
        "wrapper failed to exec the stub: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "STUB-OK --botan-shim --version"
    );
}

proptest::proptest! {
    // Round-trip for arbitrary key/nonce/plaintext — pins the ct||tag
    // framing beyond the fixed spec vectors.
    #[test]
    fn gcm_roundtrip_for_arbitrary_inputs(
        key in proptest::collection::vec(proptest::prelude::any::<u8>(), 32),
        nonce in proptest::collection::vec(proptest::prelude::any::<u8>(), 12),
        pt in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512),
    ) {
        let blob = gcm_encrypt(&key, &nonce, &pt).expect("encrypt");
        proptest::prop_assert_eq!(blob.len(), pt.len() + 16);
        let back = gcm_decrypt(&key, &nonce, &blob).expect("decrypt");
        proptest::prop_assert_eq!(back, pt);
    }

    // Encoding round-trip: any byte string survives hex encode/decode.
    #[test]
    fn hex_roundtrip_for_arbitrary_bytes(
        bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..256),
    ) {
        let text = hex(&bytes);
        proptest::prop_assert_eq!(hex_decode(&text).expect("decode"), bytes);
    }

    // Total on arbitrary text — stdin is subprocess input we don't
    // control; the decoder must reject, never panic.
    #[test]
    fn hex_decode_never_panics(s in ".*") {
        let _ = hex_decode(&s);
    }

    // Total on arbitrary argv shapes.
    #[test]
    fn parse_cipher_args_never_panics(
        args in proptest::collection::vec(".{0,40}", 0..8),
    ) {
        let _ = parse_cipher_args(&args);
    }

    // The hash op's output shape is what ani-cli pipes onward: exactly
    // 64 uppercase hex digits for any input.
    #[test]
    fn sha256_hex_upper_is_always_64_uppercase_hex(
        data in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..256),
    ) {
        let h = sha256_hex_upper(&data);
        proptest::prop_assert_eq!(h.len(), 64);
        proptest::prop_assert!(h.bytes().all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b)));
    }
}

// --- per-process wrapper isolation ---

#[test]
fn per_process_shim_dir_scopes_by_pid() {
    let got = per_process_shim_dir(std::path::Path::new("/cache"), 4242);
    assert_eq!(got, std::path::Path::new("/cache/botan-shim/4242"));
}

#[test]
fn provision_own_botan_shim_uses_a_pid_scoped_dir() {
    // Two concurrent instances must not overwrite each other's
    // wrapper: an exiting AppImage unmounts its binary path and a
    // shared wrapper would leave the surviving instance execing a
    // path that no longer exists.
    let td = tempfile::tempdir().expect("tempdir");
    let got = provision_own_botan_shim(td.path()).expect("provisioned");
    assert!(
        got.ends_with(format!("botan-shim/{}", std::process::id())),
        "dir is scoped to this process: {}",
        got.display()
    );
    assert!(got.join("botan").is_file());
}

#[test]
fn prune_keeps_the_current_and_live_dirs_and_removes_dead_ones() {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path().join("botan-shim");
    for pid in ["100", "200", "300", "not-a-pid"] {
        std::fs::create_dir_all(root.join(pid)).expect("mkdir");
    }
    let keep = root.join("200");
    prune_stale_shim_dirs(&root, &keep, |pid| pid == 300);
    assert!(!root.join("100").exists(), "dead sibling removed");
    assert!(root.join("200").exists(), "own dir kept");
    assert!(root.join("300").exists(), "live sibling kept");
    assert!(
        root.join("not-a-pid").exists(),
        "unrecognized entries left alone"
    );
}
