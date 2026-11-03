/**
 * Tell the Electron main process that the app's first page has
 * rendered: the window is revealed only then (electron/lib/app-window.cjs),
 * so a slow first render never shows the bare shell. Called once the
 * layout has mounted, after two animation frames, so the frame the
 * window will show has been drawn. Outside the app (browser-only dev,
 * tests) there is no bridge and it does nothing.
 */
type ReadyWindow = { aniGui?: { appReady?: () => void } } | undefined;

export function signalAppReady(
	win: ReadyWindow = globalThis.window,
	raf: (cb: () => void) => unknown = (cb) => requestAnimationFrame(cb)
): void {
	raf(() => raf(() => win?.aniGui?.appReady?.()));
}
