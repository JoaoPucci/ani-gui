/**
 * A test run's flags file, written for one launch.
 *
 * The packaged app under test is a test-run build: its backend is built
 * with the `test-run-key` feature (`pnpm run package:e2e`), so a test
 * run's file (`ANI_GUI_TEST_FLAGS_FILE`) whose minisign signature
 * verifies under the fixture test-run key may turn a flag on. That
 * key's secret is derived from a fixed seed, the same way
 * backend/tests/fixtures/flags-signature/make.py derives it, so it is
 * public, and no packaged release trusts it (the release-shaped job in
 * .github/workflows/e2e.yml checks the shipped binary for it).
 *
 * The signature is minisign's prehashed format: Ed25519 over the
 * file's BLAKE2b-512 hash, then over that signature and the trusted
 * comment. flags/test_run_probe.spec.ts checks that this produces the
 * backend fixture's signature byte for byte.
 */
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

/** Who signs the file: the test-run key a test-run build trusts, a key
 *  no build trusts, or nobody. */
export type Signer = 'test-run key' | 'untrusted key' | 'unsigned';

const SEEDS: Record<Exclude<Signer, 'unsigned'>, string> = {
	'test-run key': 'ani-gui test-run key',
	// Key two of the backend's fixtures: trusted by no build.
	'untrusted key': 'ani-gui test key two'
};

/** PKCS#8 DER prefix of a raw 32-byte Ed25519 private key. */
const ED25519_PKCS8 = Buffer.from('302e020100300506032b657004220420', 'hex');

function keyFor(seed: string): { key: crypto.KeyObject; id: Buffer } {
	const secret = crypto.createHash('sha256').update(seed).digest();
	const key = crypto.createPrivateKey({
		key: Buffer.concat([ED25519_PKCS8, secret]),
		format: 'der',
		type: 'pkcs8'
	});
	const id = crypto.createHash('sha256').update(`id${seed}`).digest().subarray(0, 8);
	return { key, id };
}

/** The minisign signature `signer` makes over `data`, for a file
 *  named `name` in its trusted comment. */
export function minisign(data: Buffer, signer: Exclude<Signer, 'unsigned'>, name: string): string {
	const { key, id } = keyFor(SEEDS[signer]);
	const hash = crypto.createHash('blake2b512').update(data).digest();
	const sig = crypto.sign(null, hash, key);
	const trusted = `timestamp:1793700000\tfile:${name}`;
	const global = crypto.sign(null, Buffer.concat([sig, Buffer.from(trusted)]), key);
	return [
		'untrusted comment: signature from minisign secret key',
		Buffer.concat([Buffer.from('ED'), id, sig]).toString('base64'),
		`trusted comment: ${trusted}`,
		global.toString('base64'),
		''
	].join('\n');
}

/** Write `flags` as a flags file in `dir`, with its signature beside it
 *  unless `signer` is `'unsigned'`, and return the file's path: the
 *  value for `ANI_GUI_TEST_FLAGS_FILE`. */
export function writeFlagsFile(
	dir: string,
	flags: Record<string, unknown>,
	signer: Signer
): string {
	const file = path.join(dir, 'flags.json');
	const body = Buffer.from(JSON.stringify({ flags }));
	fs.writeFileSync(file, body);
	if (signer !== 'unsigned') {
		fs.writeFileSync(`${file}.minisig`, minisign(body, signer, 'flags.json'));
	}
	return file;
}
