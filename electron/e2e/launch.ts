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
import { _electron, type ElectronApplication } from '@playwright/test';
import path from 'node:path';
import { guardPolicy } from './policy-guard';

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

/** Launch the packaged app with `env` as its whole environment. Every
 *  launch is guarded: a content security policy violation the test did
 *  not declare fails it (./policy-guard.ts). */
export async function launchPackagedApp(
	env: NonNullable<Parameters<typeof _electron.launch>[0]>['env']
): Promise<ElectronApplication> {
	const app = await _electron.launch({
		executablePath: packagedBinary,
		args: [...testLaunchSwitches],
		env
	});
	guardPolicy(app);
	return app;
}
