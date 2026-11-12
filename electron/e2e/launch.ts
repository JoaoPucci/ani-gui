/**
 * The one way an e2e spec launches the packaged app.
 *
 * Every launch here is muted. A headless launch under Xvfb hides the
 * window, not the audio: the app still opens the host's audio device,
 * and test runs have played sound on the desktop of whoever was
 * running them. Muting is easy to forget in a new spec and silent
 * when forgotten, so specs do not pass switches of their own —
 * tests/arch/e2e_launch.sh keeps Playwright's Electron driver in this
 * file alone, and smoke.spec.ts asks the running app whether the
 * switch arrived.
 *
 * Every launch here also refuses a display someone may be looking at
 * (lib/e2e-display.cjs): outside CI the suite runs inside the private
 * headless GNOME Shell session of tools/headless-wayland.sh, on
 * Wayland; in CI, on the runner's Xvfb. Test windows opened on the
 * desktop of whoever ran the suite before this, even under xvfb-run.
 * smoke.spec.ts asks the running app which platform it landed on.
 *
 * And every launch here keeps the backend off the network. A spec's
 * stubs answer only what the renderer asks after they are set, and the
 * first page asks before then. The launched app gets a proxy that
 * refuses every connection beyond this machine and records it
 * (outboundAttempts); smoke.spec.ts asks the backend for something
 * only GitHub knows and finds the attempt there.
 *
 * Pre-condition: `pnpm run package:e2e` (or the chained `pnpm e2e`)
 * has produced `dist-e2e/linux-unpacked/`.
 */
import {
	_electron,
	type BrowserContext,
	type ElectronApplication,
	type Page
} from '@playwright/test';
import type { ChildProcess } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import { launchRefusal } from '../lib/e2e-display.cjs';
import { stopOwnGroup } from '../lib/own-group.cjs';

/** The e2e package (`pnpm run package:e2e`, lib/e2e-package.cjs):
 *  electron-builder's unpacked tree of the release's Linux package —
 *  the production code path: the `app://` protocol handler, the bundled
 *  SvelteKit assets and the sidecar backend under
 *  `process.resourcesPath` — except that its backend is a test-run
 *  build, so a flags file signed by the test-run key can turn a flag
 *  on (flags-file.ts). */
export const packagedBinary = path.join(
	path.resolve(__dirname, '..'),
	'dist-e2e/linux-unpacked/ani-gui'
);

/** The switches every test launch carries.
 *
 *  `--no-sandbox` because chrome-sandbox in the unpackaged
 *  linux-unpacked dir isn't SUID; the .deb's postinst fixes that for
 *  installed copies, and these tests run the pre-install artifact.
 *
 *  `--mute-audio` so a test run makes no sound. */
export const testLaunchSwitches = ['--no-sandbox', '--mute-audio'] as const;

/** What every test launch sets, whatever the spec passes: a test
 *  run (`ANI_GUI_UNATTENDED=1`). A boot that fails then exits instead
 *  of waiting on a dialog nobody reads (lib/boot-failure.cjs), and the
 *  backend takes its flags from the test — `ANI_GUI_TEST_FLAGS_FILE`,
 *  or the defaults — and never asks GitHub for the live file, so a
 *  flipped flag or a slow CDN cannot change a test's result
 *  (backend/src/flags/launch.rs). */
export const testRunEnv = { ANI_GUI_UNATTENDED: '1' } as const;

/** A proxy that lets nothing through. It answers every request with
 *  403 and closes, and records the request line: `CONNECT host:port`
 *  for HTTPS, the absolute URL's request line for plain HTTP. */
async function refusingProxy(): Promise<{ origin: string; seen: string[]; close: () => void }> {
	const seen: string[] = [];
	const server = net.createServer((socket) => {
		let head = '';
		socket.on('data', (chunk) => {
			if (head.includes('\r\n')) return;
			head += chunk.toString('latin1');
			const end = head.indexOf('\r\n');
			if (end < 0) return;
			// `CONNECT host:port HTTP/1.1` or `GET http://… HTTP/1.1`.
			seen.push(head.slice(0, end).replace(/ HTTP\/\d(\.\d)?$/, ''));
			socket.end('HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\nconnection: close\r\n\r\n');
		});
		socket.on('error', () => socket.destroy());
	});
	await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
	// Never what keeps the test worker alive.
	server.unref();
	const { port } = server.address() as net.AddressInfo;
	return { origin: `http://127.0.0.1:${port}`, seen, close: () => server.close() };
}

/** The proxy settings every test launch carries, pointing at `origin`:
 *  the backend's HTTP clients (reqwest) and the provider transport
 *  (curl) both read them, in either case. Loopback is exempt, so the
 *  backend still reaches a spec's own local servers — a media fixture,
 *  an upstream that never answers — and the renderer its backend. */
function proxyEnv(origin: string): Record<string, string> {
	const env: Record<string, string> = {};
	for (const name of ['http_proxy', 'https_proxy', 'all_proxy']) {
		env[name] = origin;
		env[name.toUpperCase()] = origin;
	}
	const loopback = '127.0.0.1,localhost,::1';
	env.no_proxy = loopback;
	env.NO_PROXY = loopback;
	return env;
}

/** `env` without a test run's flags file. */
function withoutFlagsFile(env: Record<string, string> | undefined): Record<string, string> {
	const { ANI_GUI_TEST_FLAGS_FILE: _dropped, ...rest } = env ?? {};
	return rest;
}

/** The variable that names `file` as the test run's flags file. */
function flagsEnv(file: string | undefined): Record<string, string> {
	return file === undefined ? {} : { ANI_GUI_TEST_FLAGS_FILE: file };
}

/** Removes the scratch directory `dir`, that path alone. A failure is
 *  reported, never thrown: it would surface in whatever test is running
 *  when the app exits, or hide the launch failure being reported. */
function removeScratch(dir: string): void {
	try {
		fs.rmSync(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
	} catch (err) {
		console.warn(`e2e: could not remove the scratch profile ${dir}: ${err}`);
	}
}

/** How long the backend may go on after the app has exited: the app
 *  stops it as it quits without waiting, and its shutdown allows 3 s
 *  for requests in flight and 5 s for teardown (backend/src/shutdown.rs). */
const BACKEND_STOP_MS = 10_000;

/** Profiles removed when their app exited, removed again once their
 *  backend has had time to stop, in case it wrote there on its way out;
 *  and again as the test worker exits, if that comes first. */
const removedProfiles = new Set<string>();
process.once('exit', () => {
	for (const dir of removedProfiles) removeScratch(dir);
});

function removeAfterExit(dir: string): void {
	removeScratch(dir);
	removedProfiles.add(dir);
	setTimeout(() => {
		removeScratch(dir);
		removedProfiles.delete(dir);
	}, BACKEND_STOP_MS).unref();
}

/** Launch the packaged app with `env` as its whole environment, as a
 *  test run — unless that environment leads to a display someone may
 *  be looking at, which throws, saying how to run the suite instead.
 *  `binary` is another copy of the packaged tree's launcher, for a spec
 *  that ships the app a frontend of its own (splash-stuck.spec.ts).
 *  `profile` is the scratch directory the spec made for this launch
 *  (its HOME or XDG directories): it is removed, that path alone, once
 *  the app has exited, and again once its backend has had time to stop,
 *  or at once if the launch never starts. A worker killed before its
 *  app exits leaves the profile behind.
 *
 *  `flagsFile` is the test run's flags file (`ANI_GUI_TEST_FLAGS_FILE`,
 *  which flags-file.ts writes) and the only way to name one: a value in
 *  `env`, inherited from the shell that runs the suite or not, is
 *  dropped, so a launch without it runs with the defaults. */
export async function launchPackagedApp(
	env: NonNullable<Parameters<typeof _electron.launch>[0]>['env'],
	{
		binary = packagedBinary,
		profile,
		flagsFile
	}: { binary?: string; profile?: string; flagsFile?: string } = {}
): Promise<ElectronApplication> {
	const refusal = launchRefusal(env ?? {});
	if (refusal) {
		if (profile) removeScratch(profile);
		throw new Error(refusal);
	}
	// The launcher script execs the binary beside it in the same process;
	// resolved before the launch, so a launched app is always recorded.
	const exe = fs.realpathSync(`${binary}.bin`);
	// Whatever a spec stubs, the backend reaches no host beyond this
	// machine: its proxy refuses every connection, and says which it
	// refused (outboundAttempts). Set after the spec's environment, so
	// no spec can lift it.
	const proxy = await refusingProxy();
	let app: ElectronApplication;
	try {
		app = await _electron.launch({
			executablePath: binary,
			args: [...testLaunchSwitches],
			env: {
				...withoutFlagsFile(env),
				...flagsEnv(flagsFile),
				...proxyEnv(proxy.origin),
				...testRunEnv
			}
		});
	} catch (err) {
		proxy.close();
		if (profile) removeScratch(profile);
		throw err;
	}
	app.process().once('exit', () => {
		proxy.close();
		if (profile) removeAfterExit(profile);
	});
	launched.set(app, { child: app.process(), exe });
	refused.set(app, proxy.seen);
	return app;
}

/** Every connection the launched app tried to open to a host beyond
 *  this machine, as `CONNECT host:port` or `GET http://…`, in the
 *  order it tried them. The backend is what reads the proxy settings;
 *  Chromium may too, on some desktops. */
export function outboundAttempts(app: ElectronApplication): string[] {
	const seen = refused.get(app);
	if (!seen) throw new Error('outboundAttempts: not an app launchPackagedApp started');
	return [...seen];
}

const refused = new WeakMap<ElectronApplication, string[]>();

/** The process each launch started, recorded as it starts: Playwright
 *  stops answering `app.process()` once the app is closed, and stopApp
 *  must still tell an exited app from a running one then. */
const launched = new WeakMap<ElectronApplication, { child: ChildProcess; exe: string }>();

/** Stop an app this suite launched, and nothing else: SIGTERM to the
 *  process group Playwright created for it, SIGKILL to the same group if
 *  it is still there after `termMs`, and only while that group is
 *  provably the app's — the main process unreaped, leading its group,
 *  running the binary it was launched from (lib/own-group.cjs). Then
 *  Playwright's close, which settles once the process has gone,
 *  whatever the stop did. An app that has already exited is closed without a signal.
 *
 *  The backend is not in that group. It stops itself once the app's end
 *  of its stdin closes (lib/backend-spawn.cjs), and nothing here
 *  signals it. */
export async function stopApp(
	app: ElectronApplication,
	{ termMs = 5000, killMs = 5000 }: { termMs?: number; killMs?: number } = {}
): Promise<void> {
	const record = launched.get(app);
	if (!record) throw new Error('stopApp: not an app launchPackagedApp started');
	const { child, exe } = record;
	try {
		await stopOwnGroup(child, { exe, termMs, killMs });
	} finally {
		// Playwright's close signals nothing: it asks the app to quit and
		// waits for the process to go. A stop that refused to signal still
		// releases Playwright's side, and the refusal is what the spec
		// reports.
		await app.close();
	}
}

/** stopApp for the cold-launch retry's cleanup (lib/cold-launch.cjs),
 *  whose budget is five seconds before the relaunch goes ahead: both
 *  waits are kept inside it. */
export function stopDeadApp(app: ElectronApplication): Promise<void> {
	return stopApp(app, { termMs: 1500, killMs: 2000 });
}

/** What the launch's splash went through (lib/splash.cjs). A test run
 *  starts fast enough that the splash has come and gone before a spec
 *  can look at the app's windows, so the main process's own record is
 *  read instead. */
export function splashState(app: ElectronApplication): Promise<Record<string, boolean | string | number>> {
	return app.evaluate(() => {
		const main = (process as unknown as { mainModule: NodeJS.Module }).mainModule;
		return main.require('./lib/splash.cjs').lastSplashState();
	});
}

/** Whether `page` is the splash rather than the app's window: the main
 *  process marks the splash's BrowserWindow (lib/splash.cjs). */
async function isSplash(app: ElectronApplication, page: Page): Promise<boolean> {
	const win = await app.browserWindow(page);
	return win.evaluate((w) => (w as unknown as Record<string, unknown>).aniGuiSplash === true);
}

/** The splash's page, while it is up: a launch whose first page is
 *  late keeps it on screen long enough to look at (splash-stuck.spec.ts). */
export async function splashWindow(app: ElectronApplication, timeoutMs = 30_000): Promise<Page> {
	const deadline = Date.now() + timeoutMs;
	for (;;) {
		for (const page of app.windows()) {
			if (page.isClosed()) continue;
			if (await isSplash(app, page).catch(() => false)) return page;
		}
		if (Date.now() > deadline) throw new Error(`no splash within ${timeoutMs} ms`);
		await new Promise((resolve) => setTimeout(resolve, 25));
	}
}

/** The app's window — not the splash, which opens first and closes
 *  once the app's window shows. Specs use this where they would use
 *  Playwright's first-window accessor, which would hand them the splash. Polls the
 *  open windows rather than waiting on the `window` event, which a
 *  window opened during the first scan would slip past; a page that
 *  closes while it is checked is the splash going away. */
export async function appWindow(app: ElectronApplication, timeoutMs = 60_000): Promise<Page> {
	const deadline = Date.now() + timeoutMs;
	for (;;) {
		for (const page of app.windows()) {
			if (page.isClosed()) continue;
			const splash = await isSplash(app, page).catch(() => true);
			if (!splash) return page;
		}
		if (Date.now() > deadline) throw new Error(`no app window within ${timeoutMs} ms`);
		await new Promise((resolve) => setTimeout(resolve, 25));
	}
}

/** `context.route`, tolerant of the splash closing while it is set.
 *  The route is installed on every open page, and the splash closes
 *  moments into a test run's boot — sometimes mid-install, which
 *  Playwright reports with its closed-target error although the
 *  context and the app's window are fine. A retry installs it on what
 *  is left; a context that is really gone fails every attempt, and the
 *  error then surfaces for the cold-launch retry to judge. */
export async function routeContext(
	context: BrowserContext,
	url: Parameters<BrowserContext['route']>[0],
	handler: Parameters<BrowserContext['route']>[1]
): Promise<void> {
	for (let attempt = 0; ; attempt += 1) {
		try {
			await context.route(url, handler);
			return;
		} catch (err) {
			const closed = String(err).includes('Target page, context or browser has been closed');
			if (!closed || attempt >= 3) throw err;
		}
	}
}

/** Answers every backend API request that none of a spec's own stubs
 *  answers with a 501, and records it as `METHOD /path`. Registered
 *  before the spec's stubs, since Playwright tries the most recently
 *  registered route first: this one sees only what they leave. A spec
 *  that claims its page runs on stubs asserts the record is empty; the
 *  request itself never reaches the backend. The first mount can ask
 *  while the stubs are still being set, so a spec that remounts its
 *  page hands the record to remountFirstPage, which forgets what the
 *  first mount sent. */
export async function refuseUnstubbed(context: BrowserContext): Promise<string[]> {
	const unstubbed: string[] = [];
	await routeContext(context, /^http:\/\/127\.0\.0\.1:\d+\/api\//, (r) => {
		const req = r.request();
		unstubbed.push(`${req.method()} ${new URL(req.url()).pathname}`);
		return r.fulfill({ status: 501, contentType: 'text/plain', body: 'unstubbed in this spec' });
	});
	return unstubbed;
}

/** Mounts the app's first page again, so every request it makes goes
 *  through the routes a spec has set since launch: the first mount asks
 *  before they are in place. A goto to the same URL would be a
 *  client-side no-op, hence the bounce through about:blank. `witness`
 *  is armed once the old page is gone and resolves when a stubbed
 *  request of the new mount arrives, the proof that the stubs are live.
 *  `forget`, a refuseUnstubbed record, is emptied at the same moment:
 *  what it holds then was sent by the first mount, which may have asked
 *  before the stub that answers it was set. */
export async function remountFirstPage(
	page: Page,
	witness: () => Promise<void>,
	{ forget }: { forget?: string[] } = {}
): Promise<void> {
	await page.waitForURL((u) => u.protocol !== 'about:');
	const first = page.url();
	// Settling is normally a second or two; bounded, so a request that
	// never answers does not hold the bounce for the full timeout.
	await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});
	await page.goto('about:blank');
	forget?.splice(0);
	const live = witness();
	await page.goto(first, { waitUntil: 'domcontentloaded' });
	await live;
}
