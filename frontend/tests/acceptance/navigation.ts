// A navigation starting, as SvelteKit reports one to a page.
//
// Every scenario mocks `$app/navigation`, so `beforeNavigate` is a
// `vi.fn()` that records the callbacks the mounted pages registered.
// Calling them is what SvelteKit does the moment a navigation starts,
// before the page it leaves goes. A scenario clears the mock before it
// mounts, so only its own page hears the navigation.
//
// The navigation carries `from` and `to` as SvelteKit's does: `from`
// is the URL on screen, as the page-state stub holds it.

import { vi } from 'vitest';
import { beforeNavigate } from '$app/navigation';
import { page } from './page-state.svelte';

function report(nav: unknown): void {
	for (const [callback] of vi.mocked(beforeNavigate).mock.calls) {
		(callback as (nav: unknown) => void)(nav);
	}
}

/** The viewer follows a link to another page. */
export function startNavigation(): void {
	const from = new URL(page.url.href);
	const to = new URL('/elsewhere', from);
	report({ from: { url: from }, to: { url: to }, type: 'link', willUnload: false, cancel() {} });
}

/** A navigation that only changes the query of the page on screen —
 *  the detail page's episode pager, the player's episode switch, or a
 *  link or goto to the URL already on screen. */
export function startQueryNavigation(query: Record<string, string> = {}): void {
	const from = new URL(page.url.href);
	const to = new URL(from.href);
	for (const [k, v] of Object.entries(query)) to.searchParams.set(k, v);
	report({ from: { url: from }, to: { url: to }, type: 'goto', willUnload: false, cancel() {} });
}

/** Forget the pages earlier scenarios mounted. */
export function forgetNavigationListeners(): void {
	vi.mocked(beforeNavigate).mockClear();
}
