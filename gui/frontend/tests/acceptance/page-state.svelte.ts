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

export const page = {
	get url() {
		return url;
	},
	get params() {
		return params;
	}
};

/** Point the stub at a route param, as SvelteKit does on navigation. */
export function setParams(next: Record<string, string>): void {
	for (const k of Object.keys(params)) delete params[k];
	Object.assign(params, next);
}

/** Navigate the stub, as the topbar does when a search is submitted. */
export function setQuery(q: string): void {
	url.searchParams.set('q', q);
}
