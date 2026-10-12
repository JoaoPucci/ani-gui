// Acceptance: a choice made in the hero framing switch is stored and
// the page reloads into it.

import { describe, it, expect, vi, afterEach } from 'vitest';
import { mount, unmount } from 'svelte';

import HeroFramingSwitch from '../../src/lib/hero/HeroFramingSwitch.svelte';
import { HERO_PREFS_STORAGE_KEY } from '../../src/lib/hero/framing';

let app: ReturnType<typeof mount> | null = null;
let target: HTMLElement | null = null;

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target?.remove();
	vi.restoreAllMocks();
	localStorage.clear();
});

function mountAt(href: string) {
	(window as unknown as { happyDOM: { setURL(url: string): void } }).happyDOM.setURL(href);
	const reload = vi.spyOn(window.location, 'reload').mockImplementation(() => {});
	target = document.createElement('div');
	document.body.appendChild(target);
	app = mount(HeroFramingSwitch, {
		target,
		props: { prefs: { framing: 'top', source: 'kitsu', controls: false }, origin: 'kitsu' }
	});
	const button = (label: string) =>
		Array.from(target!.querySelectorAll('button')).find((b) => b.textContent === label)!;
	return { reload, button };
}

describe('hero framing switch', () => {
	it('stores the choice and reloads in place, fragment or not', () => {
		const { reload, button } = mountAt('http://localhost/anime/1?page=2#ep-3');
		button('pillar').click();
		expect(JSON.parse(localStorage.getItem(HERO_PREFS_STORAGE_KEY) ?? 'null')).toEqual({
			framing: 'pillar',
			source: 'kitsu',
			controls: false
		});
		expect(reload).toHaveBeenCalledTimes(1);
	});
});
