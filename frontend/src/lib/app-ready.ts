/**
 * Tell the Electron main process that the app's first page has
 * rendered. Stub.
 */
type ReadyWindow = { aniGui?: { appReady?: () => void } } | undefined;

export function signalAppReady(
	win: ReadyWindow = globalThis.window,
	raf: (cb: () => void) => unknown = (cb) => requestAnimationFrame(cb)
): void {
	void win;
	void raf;
}
