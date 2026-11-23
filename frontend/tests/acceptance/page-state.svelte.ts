// A reactive stand-in for `$app/state`'s `page`.
//
// The search route drives its query off `page.url.searchParams` inside
// an `$effect`, so a scenario that needs a SECOND search — the
// superseded-run case — has to change that URL in a way the effect
// actually notices. `SvelteURL` is the reactive URL Svelte ships for
// this; a plain one would be read once and never again.

import { SvelteURL } from 'svelte/reactivity';

const url = new SvelteURL('http://127.0.0.1:31337/search');

/** Route params. The detail route reads `page.params.id`; the home
 *  and search routes never look. */
const params = $state<Record<string, string>>({});

/** The history entry's shallow-routing state, as SvelteKit restores it
 *  onto `page.state` when the viewer steps back or forward onto it. */
let entryState = $state<App.PageState>({});

export const page = {
	get url() {
		return url;
	},
	get params() {
		return params;
	},
	get state() {
		return entryState;
	}
};

/** Point the stub at an entry's state, as SvelteKit does when it lands
 *  on that entry, or as its `replaceState` does when the page writes one. */
export function setPageState(next: App.PageState): void {
	entryState = next;
}

/** Point the stub at a route param, as SvelteKit does on navigation. */
export function setParams(next: Record<string, string>): void {
	for (const k of Object.keys(params)) delete params[k];
	Object.assign(params, next);
}

/** Navigate the stub, as the topbar does when a search is submitted. */
export function setQuery(q: string): void {
	url.searchParams.set('q', q);
}

/**
 * Point the stub at a path and query string, as a `goto` does.
 *
 * The play route carries its whole session in the URL — which episode
 * is playing, which stream session serves it — so a scenario that
 * mounts it has to say more than the route param.
 */
export function setUrl(pathname: string, query: Record<string, string> = {}): void {
	url.pathname = pathname;
	url.search = '';
	for (const [k, v] of Object.entries(query)) url.searchParams.set(k, v);
}
