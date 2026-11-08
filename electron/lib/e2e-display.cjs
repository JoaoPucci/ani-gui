// Which display an e2e launch of the packaged app may use.
//
// The app opens real windows on whatever display its environment
// names, and a suite run from a desktop shell inherits that desktop:
// test windows opened on the screen of whoever ran it, even under
// xvfb-run, where the packaged launcher still picked the session's
// Wayland compositor. So a launch is refused unless the display is one
// nobody is looking at:
//
//   - outside CI, the private headless GNOME Shell session that
//     tools/headless-wayland.sh starts. It exports
//     ANI_GUI_HEADLESS_WAYLAND, the full path of its compositor's
//     socket; the launch must lead to that socket and nowhere else —
//     no X display beside it, no inherited Wayland socket descriptor.
//   - on a GitHub Actions runner (GITHUB_ACTIONS=true, which the
//     runner sets), its Xvfb. A runner has no desktop, but a Wayland
//     session there would mean the run is not where it thinks it is, so
//     any is refused. `CI` alone is not enough: anyone can set it.
//
// Wayland socket paths are POSIX paths on every host this runs on, so
// they are joined as such (the unit tests run on Windows too).
//
// The check reads the environment the app is about to receive, not
// the test runner's own.

const path = require('node:path').posix;

const HOW = 'run the e2e suite through tools/headless-wayland.sh (see docs/testing.md)';

function isCI(env) {
	return env.GITHUB_ACTIONS === 'true';
}

/** The Wayland socket `env` leads to, or null when it names none. */
function waylandSocket(env) {
	const display = env.WAYLAND_DISPLAY;
	if (!display) return null;
	if (path.isAbsolute(display)) return display;
	if (!env.XDG_RUNTIME_DIR) return null;
	return path.join(env.XDG_RUNTIME_DIR, display);
}

/** Why a launch with `env` would reach a display someone may be
 *  looking at, or null when it may go ahead. */
function launchRefusal(env) {
	if (isCI(env)) {
		if (env.WAYLAND_DISPLAY || env.WAYLAND_SOCKET || env.XDG_SESSION_TYPE === 'wayland') {
			return (
				'refusing to launch the app: CI runs on Xvfb, but this environment carries a ' +
				'Wayland session (WAYLAND_DISPLAY, WAYLAND_SOCKET or XDG_SESSION_TYPE=wayland)'
			);
		}
		return null;
	}
	const marker = env.ANI_GUI_HEADLESS_WAYLAND;
	if (!marker) {
		return `refusing to launch the app onto this session's display: ${HOW}`;
	}
	if (env.DISPLAY) {
		return (
			`refusing to launch the app: DISPLAY=${env.DISPLAY} is set beside the headless ` +
			`compositor, and an X server reached from here may be the desktop's; ${HOW}`
		);
	}
	if (env.WAYLAND_SOCKET) {
		return `refusing to launch the app: WAYLAND_SOCKET hands it an inherited compositor; ${HOW}`;
	}
	const socket = waylandSocket(env);
	if (socket !== marker) {
		return (
			`refusing to launch the app: its Wayland display (${socket ?? 'none'}) is not the ` +
			`headless compositor's (${marker}); ${HOW}`
		);
	}
	return null;
}

/** The Ozone platform a launch with `env` must land on: Wayland in the
 *  headless session, X11 on CI's Xvfb. */
function expectedPlatform(env) {
	return env.ANI_GUI_HEADLESS_WAYLAND ? 'wayland' : 'x11';
}

module.exports = { launchRefusal, expectedPlatform };
