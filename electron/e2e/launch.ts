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
 * Pre-condition: `pnpm run package` (or the chained `pnpm e2e`) has
 * produced `dist/linux-unpacked/`.
 */
import { _electron, type ElectronApplication, type Page } from '@playwright/test';
import path from 'node:path';

/** electron-builder's pre-AppImage output — the production code path:
 *  the `app://` protocol handler, the bundled SvelteKit assets and the
 *  sidecar backend under `process.resourcesPath`. */
export const packagedBinary = path.join(
	path.resolve(__dirname, '..'),
	'dist/linux-unpacked/ani-gui'
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

/** Launch the packaged app with `env` as its whole environment, as a
 *  test run. */
export function launchPackagedApp(
	env: NonNullable<Parameters<typeof _electron.launch>[0]>['env']
): Promise<ElectronApplication> {
	return _electron.launch({
		executablePath: packagedBinary,
		args: [...testLaunchSwitches],
		env: { ...env, ...testRunEnv }
	});
}

/** Whether `page` is the splash rather than the app's window: the main
 *  process marks the splash's BrowserWindow (lib/splash.cjs). */
async function isSplash(app: ElectronApplication, page: Page): Promise<boolean> {
	const win = await app.browserWindow(page);
	return win.evaluate((w) => (w as unknown as Record<string, unknown>).aniGuiSplash === true);
}

/** The app's window — not the splash, which opens first and closes
 *  once the app's window shows. Specs use this where they would use
 *  `app.firstWindow()`, which would hand them the splash. */
export async function appWindow(app: ElectronApplication): Promise<Page> {
	for (const page of app.windows()) {
		if (!(await isSplash(app, page))) return page;
	}
	return app.waitForEvent('window', {
		predicate: async (page) => !(await isSplash(app, page))
	});
}
