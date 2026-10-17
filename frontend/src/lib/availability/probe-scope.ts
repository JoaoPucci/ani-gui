/**
 * The page an availability probe was asked for, so leaving the page
 * aborts it.
 *
 * Probes are fired by the page's own code — the search pool, the home
 * page's Continue Watching rows and retries, the detail and play
 * pages' lookups — and none of it aborted anything on the way out, so
 * a page left mid-probe kept its probes running, holding connections
 * and limiter slots for answers nobody would read.
 *
 * A probe asked without a signal of its own takes the signal of the
 * path it was asked on. After a navigation has settled, every path
 * other than the current one is aborted. Settling after the
 * navigation, not before, is what keeps the next page's probes: it
 * mounts and asks before the navigation ends, under its own path, and
 * the old page has been torn down by then — or, where the route keeps
 * its component and only a parameter changes (one show's page to
 * another's), has moved its own context on — so a probe it loses reads
 * as superseded rather than as a failure on a page still standing. A
 * navigation that keeps the path (a query change on the same page)
 * aborts nothing — which is why a surface that can start a second run
 * of probes on the same page (a new search, a lookup asked again)
 * begins a run of its own and ends it when the run is superseded.
 */

/** One run of probes on a page: a search, a lookup, a load. */
export interface ProbeRun {
	signal: AbortSignal;
	/** The run is over — superseded or done. */
	end(): void;
}

export interface ProbeScope {
	/** The signal for probes asked now, on the current path. */
	signal(): AbortSignal;
	/** Start a run of probes on the current path. */
	begin(): ProbeRun;
	/** A navigation has settled: abort every other path's probes. */
	settle(): void;
}

export function createProbeScope(currentPath: () => string): ProbeScope {
	const scopes = new Map<string, AbortController>();
	const scope: ProbeScope = {
		begin() {
			// A run is a controller of its own that the path's also
			// aborts: ending it stops its probes and nobody else's, and
			// leaving the page stops it with the rest.
			const own = new AbortController();
			const page = scope.signal();
			const leave = () => own.abort();
			if (page.aborted) own.abort();
			else page.addEventListener('abort', leave, { once: true });
			return {
				signal: own.signal,
				end: () => {
					page.removeEventListener('abort', leave);
					own.abort();
				}
			};
		},
		signal() {
			const path = currentPath();
			let scope = scopes.get(path);
			if (!scope) {
				scope = new AbortController();
				scopes.set(path, scope);
			}
			return scope.signal;
		},
		settle() {
			const path = currentPath();
			for (const [left, scope] of scopes) {
				if (left === path) continue;
				scope.abort();
				scopes.delete(left);
			}
		}
	};
	return scope;
}

export const probeScope = createProbeScope(() =>
	typeof location === 'undefined' ? '' : location.pathname
);
