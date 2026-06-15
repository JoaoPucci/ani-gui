/**
 * Reactive "the Watch Later snapshot changed, re-pull it" signal.
 *
 * `invalidateWatchLater` clears the localStorage freshness stamp, which
 * covers the case where Home mounts *after* a sync. But a fire-and-
 * forget `syncWatchedToTrackers` fired from Home can finish while Home
 * is already mounted — the rail has loaded and the stamp removal isn't
 * reactive, so the just-watched title would linger until a manual
 * refresh or remount (Codex PR #71). Bumping this rune store after the
 * sync gives Home a reactive dependency to re-pull on.
 */

class WatchLaterRefreshSignal {
	/** Monotonic counter; subscribers re-run when it changes. */
	version = $state(0);

	bump(): void {
		this.version += 1;
	}
}

export const watchLaterRefreshSignal = new WatchLaterRefreshSignal();
