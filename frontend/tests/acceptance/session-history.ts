// The window's session history, as Chromium keeps it, for scenarios
// that mount the layout with the router mocked.
//
// SvelteKit writes every entry with `history.pushState` or
// `history.replaceState` and then reports the navigation to the
// `afterNavigate` callbacks. The Navigation API's
// `navigation.currentEntry.index` follows those writes: a push moves
// it one on, a replace leaves it where it was, a back or forward press
// moves it to the entry pressed onto, and a reload keeps it. happy-dom
// has no Navigation API, so this keeps the list of entries and the
// current one, puts the index on `window.navigation`, and reports each
// step to the mocked `afterNavigate` with the type SvelteKit reports
// for it — `goto` for a replace, exactly as for a push.
//
// Each entry keeps the shallow-routing state SvelteKit keeps on it:
// what the page wrote to `page.state` while the entry was current,
// put back on `page.state` when a press lands on it. A new document (a
// launch or a reload) starts with an empty `page.state` and clears what
// its entry kept, as SvelteKit's `enter` does.

import { vi } from 'vitest';
import { afterNavigate } from '$app/navigation';
import { page, setPageState, setUrl } from './page-state.svelte';

const BASE = 'http://127.0.0.1:31337';

interface Entry {
	url: string;
	state: App.PageState;
}

let entries: Entry[] = [];
let index = 0;

function show(url: string, state: App.PageState): void {
	const u = new URL(url, BASE);
	setUrl(u.pathname, Object.fromEntries(u.searchParams));
	setPageState(state);
}

function report(type: 'enter' | 'link' | 'goto' | 'form' | 'popstate', from: string | null): void {
	const nav = {
		type,
		from: from === null ? null : { url: new URL(from, BASE) },
		to: { url: new URL(entries[index].url, BASE) },
		willUnload: false
	};
	for (const [callback] of vi.mocked(afterNavigate).mock.calls) {
		(callback as (n: unknown) => void)(nav);
	}
}

/** Keep what the page wrote on the entry being left; its address. */
function leave(): string {
	entries[index].state = { ...page.state };
	return entries[index].url;
}

function press(to: number): void {
	if (to < 0 || to >= entries.length) return;
	const from = leave();
	index = to;
	show(entries[index].url, entries[index].state);
	report('popstate', from);
}

export const sessionHistory = {
	/** Put the model on `window.navigation`, with one entry at `url`. */
	install(url = '/'): void {
		entries = [{ url, state: {} }];
		index = 0;
		Object.defineProperty(window, 'navigation', {
			configurable: true,
			value: {
				get currentEntry() {
					return { index };
				}
			}
		});
	},
	uninstall(): void {
		delete (window as { navigation?: unknown }).navigation;
		entries = [];
		index = 0;
	},
	/** The position of the current entry. */
	get index(): number {
		return index;
	},
	/** A new document on the current entry: a launch, a reload, or a
	 *  return to the app. The scenario mounts the layout first. */
	open(): void {
		// SvelteKit's `enter` writes the entry afresh, without its state.
		entries[index].state = {};
		show(entries[index].url, {});
		report('enter', null);
	},
	/** A navigation that adds an entry, dropping any forward of it. */
	push(type: 'link' | 'goto' | 'form', url: string): void {
		const from = leave();
		entries.splice(index + 1);
		entries.push({ url, state: {} });
		index += 1;
		show(url, {});
		report(type, from);
	},
	/** A `goto(url, { replaceState: true })`: the current entry becomes
	 *  `url`, and SvelteKit reports it as a `goto`. */
	replace(url: string): void {
		const from = leave();
		entries[index] = { url, state: {} };
		show(url, {});
		report('goto', from);
	},
	back(): void {
		press(index - 1);
	},
	forward(): void {
		press(index + 1);
	},
	/** The navigation a mocked `goto` stands for, given its arguments. */
	goto(url: string | URL, opts?: { replaceState?: boolean }): void {
		if (opts?.replaceState) this.replace(String(url));
		else this.push('goto', String(url));
	}
};
