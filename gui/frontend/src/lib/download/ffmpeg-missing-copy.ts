/**
 * Modal-copy selector for the "ffmpeg is missing" failure.
 *
 * The recovery instructions differ enough between platforms that one
 * shared body string ends up misleading two thirds of users:
 *   - Windows installs ffmpeg via the NSIS installer's bundled fetcher
 *     (`fetch-windows-deps.mjs`). If the user dismissed it without
 *     internet, the right answer is "re-run the installer".
 *   - Linux installs ffmpeg via the distro package manager. The
 *     `.deb` declares it as a Recommends so apt picks it up; AppImage
 *     users on a fresh distro hit this modal. Either way the recovery
 *     is one command — apt / dnf / pacman, depending on family.
 *   - macOS installs ffmpeg via Homebrew (`brew install ffmpeg`),
 *     with a manual download as the non-brew fallback.
 *
 * The action button (external link to ffmpeg.org/download) helps
 * Windows users and the Mac fallback case, but on Linux it points
 * users away from the right recovery path. Suppress it there.
 *
 * Pure / framework-agnostic — i18n resolution lives at the call site
 * so this module stays trivially testable and doesn't pull paraglide
 * into the unit-test graph.
 */

export type FfmpegMissingBodyKey = 'win32' | 'darwin' | 'linux';

export interface FfmpegMissingCopy {
	/** Discriminator the caller uses to pick the right `m.*` message. */
	bodyKey: FfmpegMissingBodyKey;
	/** Whether to render the ffmpeg.org action link next to dismiss. */
	showAction: boolean;
}

/** Maps `process.platform` (or whatever the preload bridge surfaced)
 *  to body-key + show-action. Unknown / missing values fall through
 *  to the Linux body — BSDs and other unixes share package-manager
 *  culture with Linux, and `window.aniGui.platform` being absent
 *  almost certainly means we're in a test or a contextIsolation edge
 *  case, where Linux copy is the safer default than Windows. */
export function selectFfmpegMissingCopy(platform: string | null | undefined): FfmpegMissingCopy {
	switch (platform) {
		case 'win32':
			return { bodyKey: 'win32', showAction: true };
		case 'darwin':
			return { bodyKey: 'darwin', showAction: true };
		default:
			return { bodyKey: 'linux', showAction: false };
	}
}
