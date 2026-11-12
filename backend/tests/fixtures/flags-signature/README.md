# Signed flags-file fixtures

Test vectors for `backend/src/flags/signature.rs`. The three key pairs
are throwaway test keys derived from fixed seeds in `make.py`, so their
secrets are public. `flags.json` turns the test flag `test_a` on and
leaves `test_b` off. `flags.json.minisig` is minisign's prehashed format
(Ed25519 over BLAKE2b-512, with a trusted comment) by key one,
`flags.json.by-key-two.minisig` the same file by key two, and
`flags.json.by-test-run-key.minisig` the same file by the test-run key.

Keys one and two are trusted by no build. The test-run key is trusted
only for a test run's file (`ANI_GUI_TEST_FLAGS_FILE`), and only by a
backend built with the `test-run-key` feature, which test builds enable
and no packaged build does. The live flags file is never trusted under
it.

Regenerate with `python3 -I make.py . flags.json` from this directory
(needs the `cryptography` package).
