// Acceptance (DEV-ONLY experiment): a choice made in the hero framing
// switch is stored and the page reloads into it. Delete with
// $lib/hero/framing.

import { describe, it, expect, vi, afterEach } from 'vitest';
import { mount, unmount } from 'svelte';

import HeroExperimentSwitch from '../../src/lib/hero/HeroExperimentSwitch.svelte';
import { HERO_EXPERIMENT_STORAGE_KEY } from '../../src/lib/hero/framing';

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
	const replace = vi.spyOn(window.location, 'replace').mockImplementation(() => {});
	target = document.createElement('div');
	document.body.appendChild(target);
	app = mount(HeroExperimentSwitch, {
		target,
		props: { exp: { framing: 'current', source: 'kitsu' }, origin: 'kitsu' }
	});
	const button = (label: string) =>
		Array.from(target!.querySelectorAll('button')).find((b) => b.textContent === label)!;
	return { reload, replace, button };
}

describe('hero framing switch', () => {
	it('drops the experiment’s query so it cannot override the choice', () => {
		const { reload, replace, button } = mountAt('http://localhost/anime/1?hero=focus&page=2');
		button('pillar').click();
		expect(JSON.parse(localStorage.getItem(HERO_EXPERIMENT_STORAGE_KEY) ?? 'null')).toEqual({
			framing: 'pillar',
			source: 'kitsu'
		});
		expect(replace).toHaveBeenCalledWith('http://localhost/anime/1?page=2');
		expect(reload).not.toHaveBeenCalled();
	});

	it('reloads in place when the URL carries no experiment query, fragment or not', () => {
		const { reload, replace, button } = mountAt('http://localhost/anime/1#ep-3');
		button('anilist').click();
		expect(reload).toHaveBeenCalledTimes(1);
		expect(replace).not.toHaveBeenCalled();
	});
});
