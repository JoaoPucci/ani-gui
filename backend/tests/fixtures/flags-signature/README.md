# Signed flags-file fixtures

Test vectors for `backend/src/flags/signature.rs`. The two key pairs are
throwaway test keys derived from fixed seeds in `make.py`; neither is
trusted by any build. `flags.json.minisig` is minisign's prehashed
format (Ed25519 over BLAKE2b-512, with a trusted comment) by key one,
`flags.json.by-key-two.minisig` the same file by key two.

Regenerate with `python3 -I make.py . flags.json` from this directory
(needs the `cryptography` package).
