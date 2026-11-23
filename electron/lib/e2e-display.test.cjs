// An e2e launch opens the packaged app's windows on whatever display
// its environment names. Run from a desktop session, that was the
// desktop: test windows opened on the screen of whoever ran the suite.
// Outside CI the launch helper now refuses any display but the private
// headless compositor tools/headless-wayland.sh starts; on a GitHub
// Actions runner, any Wayland session at all, since CI runs on Xvfb.

const test = require('node:test');
const assert = require('node:assert/strict');

const { launchRefusal, expectedPlatform } = require('./e2e-display.cjs');

const rt = '/tmp/ani-gui-wl.abc123';
const headless = {
	XDG_RUNTIME_DIR: rt,
	WAYLAND_DISPLAY: 'ani-gui-e2e',
	XDG_SESSION_TYPE: 'wayland',
	ANI_GUI_HEADLESS_WAYLAND: `${rt}/ani-gui-e2e`
};
const desktop = {
	XDG_RUNTIME_DIR: '/run/user/1000',
	WAYLAND_DISPLAY: 'wayland-0',
	DISPLAY: ':0',
	XDG_SESSION_TYPE: 'wayland'
};

test('the private headless compositor is allowed', () => {
	assert.equal(launchRefusal(headless), null);
	// libwayland also takes WAYLAND_DISPLAY as the socket's full path.
	assert.equal(launchRefusal({ ...headless, WAYLAND_DISPLAY: `${rt}/ani-gui-e2e` }), null);
});

test('the desktop session is refused, naming the way to run the suite', () => {
	const refusal = launchRefusal(desktop);
	assert.match(refusal, /tools\/headless-wayland\.sh/);
});

test('a session that names another compositor than the marker is refused', () => {
	assert.notEqual(launchRefusal({ ...headless, WAYLAND_DISPLAY: 'wayland-0' }), null);
	assert.notEqual(
		launchRefusal({ ...headless, XDG_RUNTIME_DIR: '/run/user/1000' }),
		null,
		'the same socket name under the desktop runtime directory is the desktop'
	);
	assert.notEqual(launchRefusal({ ...headless, WAYLAND_DISPLAY: undefined }), null);
	assert.notEqual(launchRefusal({ ...headless, XDG_RUNTIME_DIR: undefined }), null);
});

test('an X display beside the headless compositor is refused', () => {
	// Chromium can fall back to X11, and an X display reached from a
	// desktop shell is the desktop's.
	assert.match(launchRefusal({ ...headless, DISPLAY: ':0' }), /DISPLAY/);
});

test('an inherited Wayland socket descriptor is refused', () => {
	assert.match(launchRefusal({ ...headless, WAYLAND_SOCKET: '5' }), /WAYLAND_SOCKET/);
});

test('no display variables at all is refused outside CI', () => {
	assert.notEqual(launchRefusal({}), null);
});

const runner = { GITHUB_ACTIONS: 'true', CI: 'true', DISPLAY: ':99' };

test('CI runs on Xvfb, and any Wayland session there is refused', () => {
	assert.equal(launchRefusal(runner), null);
	assert.notEqual(launchRefusal({ ...runner, WAYLAND_DISPLAY: 'wayland-0' }), null);
	assert.notEqual(launchRefusal({ ...runner, WAYLAND_SOCKET: '5' }), null);
	assert.notEqual(
		launchRefusal({ ...runner, XDG_SESSION_TYPE: 'wayland' }),
		null,
		'a Wayland session type is enough for Chromium to look for the default socket'
	);
});

test('CI set by hand is not a runner: a desktop X display stays refused', () => {
	assert.notEqual(launchRefusal({ CI: 'true', DISPLAY: ':0' }), null);
	assert.notEqual(launchRefusal({ CI: '1', DISPLAY: ':0' }), null);
	assert.notEqual(launchRefusal({ GITHUB_ACTIONS: 'false', DISPLAY: ':0' }), null);
});

test('the platform a launch must land on: Wayland in the headless session, X11 otherwise', () => {
	assert.equal(expectedPlatform(headless), 'wayland');
	assert.equal(expectedPlatform(runner), 'x11');
});
