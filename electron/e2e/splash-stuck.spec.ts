/**
 * A launch whose first page is late never reveals an empty window: 15 s
 * after the splash painted, the splash says the start is taking longer
 * than usual and offers Restart and Quit (lib/splash-stuck.cjs).
 *
 * The late page is real. Each test runs a copy of the packaged tree,
 * hard-linked from `dist/linux-unpacked/`, whose `resources/frontend/
 * build` holds a fixture page instead of the app's frontend
 * (e2e/fixtures/first-page/): one that never says it rendered, one
 * that says so 20 s after it loaded. Everything else — the splash, the
 * main process, the backend, the app:// handler — is the packaged
 * app's own, and nothing in it is told this is a test beyond what
 * every e2e launch sets (launch.ts).
 *
 * Restart's relaunch is checked as the person would see it: this
 * process exits, and a new one from the same tree starts and keeps
 * running, holding the single-instance lock rather than handing over to
 * the one that is exiting.
 *
 * Processes: a test signals only what it can prove it started. The
 * process Playwright launched is stopped through stopApp (launch.ts):
 * its own process group, while provably the app's. The
 * relaunched one is outside Playwright's reach, and outside this
 * test's process tree too: its parent exits, and it is reparented to
 * the session's subreaper, the user's service manager, which must
 * never be signalled. So it is never looked for in the process list
 * and its parent is never touched. It names itself: Chromium's
 * single-instance lock in the test's own temporary profile is a
 * symlink to `<host>-<pid>` of the process holding it. That pid is
 * signalled only after /proc/<pid>/exe shows it running this test's
 * own copy of the binary, and only that pid: its own quit stops its
 * backend.
 *
 * Everything the app could reach is the test's: HOME and the XDG
 * directories are a temporary directory, and the display is the run's
 * own — the private session tools/headless-wayland.sh starts locally,
 * with its own session bus, or CI's Xvfb — which the relaunched
 * process inherits.
 */
import { expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import fs, { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { launchPackagedApp, packagedBinary, splashState, splashWindow, stopApp } from './launch';

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`);
	}
});

const copies = path.join(path.dirname(path.dirname(packagedBinary)), 'e2e-splash-stuck');

test.afterAll(() => {
	fs.rmSync(copies, { recursive: true, force: true });
});

/** A copy of the packaged tree whose first page is the fixture `page`;
 *  returns its launcher. Hard links, so it costs no space: the copy's
 *  frontend directory is removed (its links only) and written anew. */
function packagedWithPage(name: string, page: 'never' | 'late'): string {
	const root = path.join(copies, name);
	fs.rmSync(root, { recursive: true, force: true });
	fs.mkdirSync(copies, { recursive: true });
	execFileSync('cp', ['-al', path.dirname(packagedBinary), root]);
	const build = path.join(root, 'resources', 'frontend', 'build');
	fs.rmSync(build, { recursive: true, force: true });
	fs.cpSync(path.join(__dirname, 'fixtures', 'first-page', page), build, { recursive: true });
	return path.join(root, path.basename(packagedBinary));
}

/** A fresh profile, in `locale` when one is given. */
function profileEnv(locale?: string) {
	const tmp = path.join(os.tmpdir(), `ani-gui-splash-stuck-${process.pid}-${Date.now()}`);
	const config = path.join(tmp, 'config');
	if (locale) {
		fs.mkdirSync(path.join(config, 'ani-gui'), { recursive: true });
		fs.writeFileSync(path.join(config, 'ani-gui', 'config.toml'), `locale = "${locale}"\n`);
	}
	fs.mkdirSync(path.join(tmp, 'home'), { recursive: true });
	return {
		tmp,
		config,
		env: {
			...process.env,
			HOME: path.join(tmp, 'home'),
			XDG_STATE_HOME: path.join(tmp, 'state'),
			XDG_CONFIG_HOME: config,
			XDG_CACHE_HOME: path.join(tmp, 'cache'),
			XDG_DATA_HOME: path.join(tmp, 'data')
		}
	};
}

/** What the main process's windows are doing. */
function windows(app: ElectronApplication) {
	return app.evaluate(({ BrowserWindow }) =>
		BrowserWindow.getAllWindows()
			.filter((w) => !w.isDestroyed())
			.map((w) => ({
				splash: (w as unknown as Record<string, unknown>).aniGuiSplash === true,
				visible: w.isVisible(),
				onTop: w.isAlwaysOnTop()
			}))
	);
}

/** The stuck state as the page shows it. */
async function stuckView(splash: Page) {
	return splash.evaluate(() => ({
		line: document.querySelector('.status')?.textContent,
		buttons: [...document.querySelectorAll('.actions button')].map((b) => b.textContent),
		label: document.querySelector('main')?.getAttribute('aria-label'),
		lang: document.documentElement.lang,
		late: document.querySelectorAll('.rail i.late').length
	}));
}

const focusedAct = (splash: Page) =>
	splash.evaluate(() => (document.activeElement as HTMLElement | null)?.dataset?.act ?? document.activeElement?.tagName);

/** The pid holding the app's single-instance lock in the profile under
 *  `config`, or null: Chromium's lock is a symlink to `<host>-<pid>`. */
function lockHolder(config: string): number | null {
	try {
		const target = fs.readlinkSync(path.join(config, 'ani-gui', 'SingletonLock'));
		const pid = Number(target.slice(target.lastIndexOf('-') + 1));
		return Number.isInteger(pid) && pid > 1 ? pid : null;
	} catch {
		return null;
	}
}

/** Whether `pid` is alive and running this test's own copy of the app
 *  (`binary`'s tree), by /proc/<pid>/exe. Nothing that fails this is
 *  ever signalled. */
function ours(pid: number | null | undefined, binary: string): pid is number {
	if (!Number.isInteger(pid) || (pid as number) <= 1) return false;
	try {
		return fs.readlinkSync(`/proc/${pid}/exe`) === `${binary}.bin`;
	} catch {
		return false;
	}
}

/** Stop the app process `pid`, if it is ours: SIGTERM, which quits it
 *  the normal way (its backend with it), then SIGKILL to the same pid
 *  if it is still there and still ours. Never a group, a child or a
 *  parent. */
async function stopOurs(pid: number | null | undefined, binary: string) {
	if (!ours(pid, binary)) return;
	process.kill(pid, 'SIGTERM');
	for (let i = 0; i < 100 && ours(pid, binary); i++) await new Promise((r) => setTimeout(r, 100));
	if (ours(pid, binary)) process.kill(pid, 'SIGKILL');
}

test('a first page that never says it rendered: the splash offers Restart and Quit, and Quit ends the launch revealing nothing', async () => {
	test.setTimeout(90_000);
	const binary = packagedWithPage('never-en', 'never');
	const { tmp, config, env } = profileEnv();
	const app = await launchPackagedApp(env, { binary });
	const pid = app.process().pid!;
	const exited = new Promise<number | null>((resolve) => app.process().once('exit', (code) => resolve(code)));
	try {
		const splash = await splashWindow(app);
		const seenAt = Date.now();
		const actions = splash.locator('.actions');
		await expect(actions).toBeHidden();
		await expect(actions).toBeVisible({ timeout: 30_000 });
		// 15 s from the paint, which was before the splash could be found.
		expect(Date.now() - seenAt).toBeGreaterThan(12_000);
		expect(await stuckView(splash)).toEqual({
			line: 'Taking longer than usual to start.',
			buttons: ['Restart', 'Quit'],
			label: 'ani-gui is starting',
			lang: 'en',
			late: 1
		});
		await expect(splash.getByRole('button', { name: 'Restart' })).toBeVisible();
		await expect(splash.getByRole('button', { name: 'Quit' })).toBeVisible();
		// The splash no longer sits above everything; the app's window is
		// still hidden, and stays so.
		expect(await windows(app)).toEqual(
			expect.arrayContaining([
				{ splash: true, visible: true, onTop: false },
				{ splash: false, visible: false, onTop: false }
			])
		);
		expect(await splashState(app)).toMatchObject({ stuck: true, choice: null, steps: 2 });

		// Nothing is focused for the person; Tab reaches Restart, then
		// Quit; Escape does nothing.
		expect(await focusedAct(splash)).toBe('BODY');
		await splash.keyboard.press('Tab');
		expect(await focusedAct(splash)).toBe('restart');
		await splash.keyboard.press('Tab');
		expect(await focusedAct(splash)).toBe('quit');
		await splash.keyboard.press('Escape');
		// Past 15 s from the app window's creation too whenever the
		// backend and the flags took under about four seconds after the
		// paint, as they do in a test run.
		await splash.waitForTimeout(4_000);
		expect(ours(pid, binary)).toBe(true);
		// Still the stuck splash, and the app's window still hidden, well
		// past the 15 s from the window's creation that used to reveal it.
		expect(await windows(app)).toEqual(
			expect.arrayContaining([
				{ splash: true, visible: true, onTop: false },
				{ splash: false, visible: false, onTop: false }
			])
		);
		expect(await windows(app)).toHaveLength(2);

		// Enter on Quit: the launch ends as closing the splash ends it.
		// A button acts on Enter's keydown, so the page closes before the
		// key comes back up, and the press reports its page gone; the exit
		// below is the check.
		await splash.keyboard.press('Enter').catch((err) => {
			if (!String(err).includes('has been closed')) throw err;
		});
		expect(await exited).toBe(0);
		// Nothing started again: no process of this tree took the lock.
		await new Promise((r) => setTimeout(r, 3_000));
		expect(ours(lockHolder(config), binary)).toBe(false);
	} finally {
		await stopApp(app);
		await stopOurs(lockHolder(config), binary);
		fs.rmSync(tmp, { recursive: true, force: true });
	}
});

test('Restart, in the app’s language, starts the app again: this process exits and a new one takes over', async () => {
	test.setTimeout(90_000);
	const binary = packagedWithPage('never-pt', 'never');
	const { tmp, config, env } = profileEnv('pt-BR');
	const app = await launchPackagedApp(env, { binary });
	const pid = app.process().pid!;
	const exited = new Promise<number | null>((resolve) => app.process().once('exit', (code) => resolve(code)));
	let relaunched: number | null = null;
	try {
		const splash = await splashWindow(app);
		await expect(splash.locator('.actions')).toBeVisible({ timeout: 30_000 });
		expect(await stuckView(splash)).toEqual({
			line: 'Está demorando mais que o normal.',
			buttons: ['Reiniciar', 'Sair'],
			label: 'O ani-gui está iniciando',
			lang: 'pt-BR',
			late: 1
		});
		expect(lockHolder(config)).toBe(pid);
		await splash.getByRole('button', { name: 'Reiniciar' }).click();
		expect(await exited).toBe(0);
		// A new process of this tree holds the lock.
		await expect
			.poll(
				() => {
					const holder = lockHolder(config);
					relaunched = holder !== pid && ours(holder, binary) ? holder : null;
					return relaunched !== null;
				},
				{ timeout: 15_000 }
			)
			.toBe(true);
		// And keeps it: it did not hand over to the process that was
		// exiting and quit at once.
		await new Promise((resolve) => setTimeout(resolve, 5_000));
		expect(lockHolder(config)).toBe(relaunched);
		expect(ours(relaunched, binary)).toBe(true);
	} finally {
		await stopOurs(relaunched, binary);
		await stopApp(app);
		// A relaunch that took the lock after the poll gave up is still
		// this test's: the lock in its own profile names it.
		await stopOurs(lockHolder(config), binary);
		fs.rmSync(tmp, { recursive: true, force: true });
	}
});

test('a first page that renders after the stuck state hands over as usual: the window is revealed and the splash goes', async () => {
	test.setTimeout(90_000);
	const binary = packagedWithPage('late', 'late');
	const { tmp, env } = profileEnv();
	const app = await launchPackagedApp(env, { binary });
	const pid = app.process().pid!;
	try {
		const splash = await splashWindow(app);
		await expect(splash.locator('.actions')).toBeVisible({ timeout: 30_000 });
		await expect
			.poll(async () => JSON.stringify(await windows(app)), { timeout: 30_000 })
			.toBe(JSON.stringify([{ splash: false, visible: true, onTop: false }]));
		expect(await splashState(app)).toMatchObject({ stuck: true, choice: null, closed: true, steps: 3 });
		const ready = await app.evaluate(() => {
			const main = (process as unknown as { mainModule: NodeJS.Module }).mainModule;
			return main.require('./lib/app-window.cjs').lastAppReady();
		});
		expect(ready).toEqual({ rendered: true, gaveUp: false, beforeReveal: true });
	} finally {
		await stopApp(app);
		fs.rmSync(tmp, { recursive: true, force: true });
	}
});
