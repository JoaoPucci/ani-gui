/**
 * The flags mechanism end to end, on the packaged app, in each state:
 * `test_run_probe`, the flag only a test-run build declares
 * (backend/src/flags), gates nothing, so what is asserted is what every
 * feature reads — the flags the window was opened with, through the
 * same `window.aniGui.flags` the renderer's `flagOn` reads.
 *
 * Off is the default, and any read failure means off: no file named, a
 * file named but missing, a file whose `true` nobody signed, and one
 * signed by a key no build trusts all leave it off. On is a file signed
 * by the test-run key, which only a test-run build trusts.
 *
 * Every declared flag has a spec like this one, named after it, that
 * launches the app with the flag off and with it on and asserts what
 * each state shows (backend/src/flags/declared_test.rs holds that the
 * file is there).
 *
 * Pre-condition: `pnpm run package:e2e` has produced
 * `dist-e2e/linux-unpacked/`.
 */
import { expect, test } from '@playwright/test';
import fs, { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { minisign, writeFlagsFile, type Signer } from '../flags-file';
import { appWindow, launchPackagedApp, packagedBinary, stopApp } from '../launch';

const FLAG = 'test_run_probe';

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package:e2e`);
	}
});

/** A scratch profile for one launch: its HOME and XDG directories. */
function scratchProfile(): { dir: string; env: Record<string, string> } {
	const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ani-gui-e2e-flags-'));
	const env: Record<string, string> = { HOME: dir };
	for (const [name, sub] of [
		['XDG_CONFIG_HOME', 'config'],
		['XDG_CACHE_HOME', 'cache'],
		['XDG_STATE_HOME', 'state'],
		['XDG_DATA_HOME', 'data']
	] as const) {
		env[name] = path.join(dir, sub);
	}
	return { dir, env };
}

/** Launch the app with the flags file `file` returns for the profile's
 *  directory (or none), and read what the window was opened with. */
async function launchWith(
	file: (dir: string) => string | null
): Promise<{ origin: unknown; flags: unknown }> {
	const { dir, env } = scratchProfile();
	const named = file(dir);
	const app = await launchPackagedApp(
		{
			...(process.env as Record<string, string>),
			...env,
			...(named === null ? {} : { ANI_GUI_TEST_FLAGS_FILE: named })
		},
		{ profile: dir }
	);
	try {
		const page = await appWindow(app);
		return await page.evaluate(() => ({
			origin: window.aniGui?.flagsOrigin,
			flags: window.aniGui?.flags
		}));
	} finally {
		await stopApp(app);
	}
}

const signed = (signer: Signer) => (dir: string) => writeFlagsFile(dir, { [FLAG]: true }, signer);

test('test_run_probe off: no flags file named, the default', async () => {
	expect(await launchWith(() => null)).toEqual({ origin: 'test', flags: { [FLAG]: false } });
});

test('test_run_probe off: the named file cannot be read', async () => {
	const seen = await launchWith((dir) => path.join(dir, 'missing.json'));
	expect(seen).toEqual({ origin: 'test', flags: { [FLAG]: false } });
});

test('test_run_probe off: a file turns it on, but nobody signed it', async () => {
	expect(await launchWith(signed('unsigned'))).toEqual({ origin: 'test', flags: { [FLAG]: false } });
});

test('test_run_probe off: a file turns it on, signed by a key no build trusts', async () => {
	const seen = await launchWith(signed('untrusted key'));
	expect(seen).toEqual({ origin: 'test', flags: { [FLAG]: false } });
});

test('test_run_probe on: a file turns it on, signed by the test-run key', async () => {
	const seen = await launchWith(signed('test-run key'));
	expect(seen).toEqual({ origin: 'test', flags: { [FLAG]: true } });
});

test("the suite's signer makes the backend fixtures' signatures", () => {
	// The on state above proves the signer satisfies the backend; this
	// pins it to make.py's output, so the two derivations of the
	// fixture keys cannot drift apart unnoticed.
	const fixtures = path.resolve(__dirname, '../../../backend/tests/fixtures/flags-signature');
	const body = fs.readFileSync(path.join(fixtures, 'flags.json'));
	for (const [signer, sig] of [
		['test-run key', 'flags.json.by-test-run-key.minisig'],
		['untrusted key', 'flags.json.by-key-two.minisig']
	] as const) {
		expect(minisign(body, signer, 'flags.json')).toBe(
			fs.readFileSync(path.join(fixtures, sig), 'utf8')
		);
	}
});
