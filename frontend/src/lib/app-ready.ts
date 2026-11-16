/**
 * Tell the Electron main process that the app's first page has
 * rendered: the window is revealed only then (electron/lib/app-window.cjs),
 * so a slow first render never shows the bare shell. Called once the
 * layout has mounted, after two animation frames, so the frame the
 * window will show has been drawn. Outside the app (browser-only dev,
 * tests) there is no bridge and it does nothing.
 *
 * It first says the page has mounted, at once: a hidden window may get
 * no frames at all (native Wayland without GPU compositing), and the
 * main process maps a window whose rendered word does not follow the
 * mounted one soon enough, so its frames resume.
 */
type ReadyWindow = { aniGui?: { appMounted?: () => void; appReady?: () => void } } | undefined;

export function signalAppReady(
	win: ReadyWindow = globalThis.window,
	raf: (cb: () => void) => unknown = (cb) => requestAnimationFrame(cb)
): void {
	win?.aniGui?.appMounted?.();
	raf(() => raf(() => win?.aniGui?.appReady?.()));
}
