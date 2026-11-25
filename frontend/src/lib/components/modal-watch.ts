/**
 * Who hears a modal open or close ($./modal-layer calls `modalsChanged`
 * on each, and re-exports `watchModals`): a feature tip's balloon
 * yields to a dialog opened over it.
 */
const watchers = new Set<() => void>();

/** Tell every watcher a modal opened or closed. */
export function modalsChanged(): void {
	for (const w of [...watchers]) w();
}

/** Call `listener` whenever a modal opens or closes, until the returned
 *  function is called. */
export function watchModals(listener: () => void): () => void {
	watchers.add(listener);
	return () => {
		watchers.delete(listener);
	};
}
