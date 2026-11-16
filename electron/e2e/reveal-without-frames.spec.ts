/**
 * The app's window opens where it gets no frames while hidden.
 *
 * The window is made hidden and revealed once its page says it has
 * rendered, which takes two animation frames (lib/app-window.cjs). Under
 * native Wayland with software compositing — a virtual machine, a GPU
 * that failed or is blocklisted, `--disable-gpu` — a window that is not
 * on screen gets no frames after its first, so that word never came,
 * the splash went stuck at 15 s and the boot failed at the first-show
 * deadline. The page now says it has mounted first, which needs no
 * frame, and a window whose rendered word does not follow is mapped
 * behind the splash so its frames resume (frameWake, wakeHidden).
 *
 * What each run proves: locally the suite runs on Wayland, in
 * tools/headless-wayland.sh's GNOME Shell session, where a hidden window
 * under `--disable-gpu` really gets no frames: there the first test
 * fails without the wake, and asserts that the wake is what opened the
 * window. CI runs on Xvfb, where X11 keeps hidden windows ticking with
 * or without a GPU: there it checks only that a launch without GPU
 * acceleration opens, and cannot see the Wayland defect.
 */
import { type ElectronApplication } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { expectedPlatform } from '../lib/e2e-display.cjs';
import { launchPackagedApp, packagedBinary, splashState, stopApp } from './launch';
import { expect, test } from './policy-guard';

test.beforeAll(() => {
	if (!fs.existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`);
	}
});

/** A fresh profile: HOME and the XDG directories under one scratch dir. */
function profileEnv() {
	const tmp = path.join(os.tmpdir(), `ani-gui-reveal-${process.pid}-${Date.now()}`);
	fs.mkdirSync(path.join(tmp, 'home'), { recursive: true });
	return {
		tmp,
		env: {
			...process.env,
			HOME: path.join(tmp, 'home'),
			XDG_STATE_HOME: path.join(tmp, 'state'),
			XDG_CONFIG_HOME: path.join(tmp, 'config'),
			XDG_CACHE_HOME: path.join(tmp, 'cache'),
			XDG_DATA_HOME: path.join(tmp, 'data')
		}
	};
}

/** What the main process saw of the reveal: the renderer's words and the
 *  wake (lib/app-window.cjs), and the windows still open. */
function revealRecord(app: ElectronApplication) {
	return app.evaluate(({ BrowserWindow }) => {
		const main = (process as unknown as { mainModule: NodeJS.Module }).mainModule;
		const appWindow = main.require('./lib/app-window.cjs');
		return {
			ready: appWindow.lastAppReady(),
			// Absent from a build that predates the wake.
			wake: typeof appWindow.lastWake === 'function' ? appWindow.lastWake() : null,
			windows: BrowserWindow.getAllWindows()
				.filter((w) => !w.isDestroyed())
				.map((w) => ({
					splash: (w as unknown as Record<string, unknown>).aniGuiSplash === true,
					visible: w.isVisible(),
					maximized: w.isMaximized()
				}))
		};
	});
}

/** Waits for the boot to be done: the page rendered, the splash closed
 *  by the boot, the app's window alone and on screen. Well inside the
 *  15 s after which a page that has not rendered leaves the splash stuck. */
async function bootDone(app: ElectronApplication) {
	await expect
		.poll(
			async () => {
				const r = await revealRecord(app);
				return r.ready.rendered && r.windows.length === 1 && !r.windows[0].splash && r.windows[0].visible;
			},
			{ timeout: 14_000 }
		)
		.toBe(true);
	expect((await splashState(app)).closed).toBe(true);
	return revealRecord(app);
}

test('without GPU acceleration the window still opens: the boot completes and the splash hands over', async () => {
	const { tmp, env } = profileEnv();
	const app = await launchPackagedApp(env, { profile: tmp, switches: ['--disable-gpu'] });
	try {
		const r = await bootDone(app);
		expect(r.ready).toEqual({ rendered: true, gaveUp: false, beforeReveal: true });
		expect(r.windows).toEqual([{ splash: false, visible: true, maximized: true }]);
		// On Wayland the hidden window got no frames: the wake is what let
		// its page render. Xvfb keeps hidden windows ticking (see above).
		if (expectedPlatform(process.env) === 'wayland') {
			expect(r.wake).toEqual({ mounted: true, woken: true });
		}
	} finally {
		await stopApp(app);
	}
});

/** Whether Chromium composites on the GPU in this run: only then does
 *  a hidden window get its frames on Wayland. X11 keeps them coming
 *  either way (CI's Xvfb has no GPU). */
async function hiddenWindowsGetFrames(app: ElectronApplication): Promise<boolean> {
	if (expectedPlatform(process.env) === 'x11') return true;
	const status = await app.evaluate(({ app: electronApp }) => electronApp.getGPUFeatureStatus());
	return String(status.gpu_compositing).startsWith('enabled');
}

test('where hidden windows get their frames the window is revealed from hidden, never woken', async () => {
	const { tmp, env } = profileEnv();
	const app = await launchPackagedApp(env, { profile: tmp });
	try {
		const r = await bootDone(app);
		expect(r.ready).toEqual({ rendered: true, gaveUp: false, beforeReveal: true });
		expect(r.wake.mounted).toBe(true);
		// A machine whose GPU Chromium cannot use (a VM) is the defect's own
		// condition even without --disable-gpu: there the wake is right.
		if (await hiddenWindowsGetFrames(app)) expect(r.wake.woken).toBe(false);
		expect(r.windows).toEqual([{ splash: false, visible: true, maximized: true }]);
	} finally {
		await stopApp(app);
	}
});
