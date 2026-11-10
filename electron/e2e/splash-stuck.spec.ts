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
 * running (it holds the single-instance lock rather than handing over
 * to the one that is exiting). The new process is outside Playwright's
 * reach, so it is found by its binary's path and stopped by hand.
 */
import { expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import fs, { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { killTree } from '../lib/cold-launch.cjs';
import { launchPackagedApp, packagedBinary, splashState, splashWindow } from './launch';

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
	return {
		tmp,
		env: {
			...process.env,
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

/** The main processes started from `binary`'s tree: not Chromium's
 *  helpers, which carry `--type=`. */
function mainProcesses(binary: string): number[] {
	const bin = `${binary}.bin`;
	return execFileSync('ps', ['-eo', 'pid=,args='], { encoding: 'utf8' })
		.split('\n')
		.map((line) => line.trim())
		.filter((line) => line.includes(bin) && !line.includes('--type='))
		.map((line) => Number(line.split(/\s+/)[0]));
}

function alive(pid: number): boolean {
	try {
		process.kill(pid, 0);
		return true;
	} catch {
		return false;
	}
}

test('a first page that never says it rendered: the splash offers Restart and Quit, and Quit ends the launch revealing nothing', async () => {
	test.setTimeout(90_000);
	const binary = packagedWithPage('never-en', 'never');
	const { tmp, env } = profileEnv();
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
		await splash.waitForTimeout(1_000);
		expect(alive(pid)).toBe(true);
		expect(await windows(app)).toHaveLength(2);

		// Enter on Quit: the launch ends as closing the splash ends it.
		await splash.keyboard.press('Enter');
		expect(await exited).toBe(0);
		expect(mainProcesses(binary)).toEqual([]);
	} finally {
		if (alive(pid)) {
			killTree(pid);
			await app.close().catch(() => {});
		}
		fs.rmSync(tmp, { recursive: true, force: true });
	}
});

test('Restart, in the app’s language, starts the app again: this process exits and a new one takes over', async () => {
	test.setTimeout(90_000);
	const binary = packagedWithPage('never-pt', 'never');
	const { tmp, env } = profileEnv('pt-BR');
	const app = await launchPackagedApp(env, { binary });
	const pid = app.process().pid!;
	const exited = new Promise<number | null>((resolve) => app.process().once('exit', (code) => resolve(code)));
	let relaunched: number[] = [];
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
		await splash.getByRole('button', { name: 'Reiniciar' }).click();
		expect(await exited).toBe(0);
		await expect
			.poll(() => (relaunched = mainProcesses(binary).filter((p) => p !== pid)), { timeout: 15_000 })
			.toHaveLength(1);
		// It kept running: it holds the lock, rather than handing over to
		// the process that was exiting and quitting at once.
		await new Promise((resolve) => setTimeout(resolve, 5_000));
		expect(relaunched.every(alive)).toBe(true);
	} finally {
		for (const p of relaunched) killTree(p);
		if (alive(pid)) {
			killTree(pid);
			await app.close().catch(() => {});
		}
		fs.rmSync(tmp, { recursive: true, force: true });
	}
});

test('a first page that renders after the stuck state hands over as usual: the window is revealed and the splash goes', async () => {
	test.setTimeout(90_000);
	const binary = packagedWithPage('late', 'late');
	const { tmp, env } = profileEnv();
	const app = await launchPackagedApp(env, { binary });
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
		await app.close();
		fs.rmSync(tmp, { recursive: true, force: true });
	}
});
