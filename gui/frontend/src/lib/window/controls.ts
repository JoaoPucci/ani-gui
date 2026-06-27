/**
 * Custom window-control helpers for the frameless Electron shell.
 *
 * The window is frameless (gui/electron/main.js `frame: false`) because
 * under native Wayland/Ozone GNOME gives Chromium no server-side
 * decorations, and Chromium's own CSD ignored GNOME's button-layout and
 * drew the controls on the left (electron/electron#48422). We draw the
 * titlebar controls ourselves; this module is the pure, testable part.
 */

/** Which side the minimize/maximize/close buttons belong on for a given
 *  `process.platform`. macOS keeps them on the left (traffic-light
 *  convention); Windows and Linux/GNOME put them on the right. Unknown or
 *  missing platforms default to the right — the app ships for Linux and
 *  Windows, both right-handed. */
export function windowControlsSide(platform: string | undefined): 'left' | 'right' {
	return platform === 'darwin' ? 'left' : 'right';
}
