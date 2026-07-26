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
