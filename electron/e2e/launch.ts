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
 * Pre-condition: `pnpm run package` (or the chained `pnpm e2e`) has
 * produced `dist/linux-unpacked/`.
 */
import { _electron, type ElectronApplication } from '@playwright/test';
import path from 'node:path';
import { launchRefusal } from '../lib/e2e-display.cjs';

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

/** Launch the packaged app with `env` as its whole environment —
 *  unless that environment leads to a display someone may be looking
 *  at, which throws, saying how to run the suite instead. */
export function launchPackagedApp(
	env: NonNullable<Parameters<typeof _electron.launch>[0]>['env']
): Promise<ElectronApplication> {
	const refusal = launchRefusal(env ?? {});
	if (refusal) throw new Error(refusal);
	return _electron.launch({
		executablePath: packagedBinary,
		args: [...testLaunchSwitches],
		env
	});
}
