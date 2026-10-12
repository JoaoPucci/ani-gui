// Acceptance: the hero framing switch names its choices in the user's
// language, shows which is in force, says which image is on screen,
// and hands a choice to the page that owns it instead of reloading.

import { describe, it, expect, vi, afterEach } from 'vitest';
import { mount, unmount } from 'svelte';

import HeroFramingSwitch from '../../src/lib/hero/HeroFramingSwitch.svelte';
import {
	HERO_PREFS_STORAGE_KEY,
	type HeroOrigin,
	type HeroPrefs
} from '../../src/lib/hero/framing';
import { m } from '../../src/lib/paraglide/messages';

let app: ReturnType<typeof mount> | null = null;
let target: HTMLElement | null = null;

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target?.remove();
	vi.restoreAllMocks();
	localStorage.clear();
});

const PREFS: HeroPrefs = { framing: 'top', source: 'kitsu', controls: true };

function mountSwitch(origin: HeroOrigin | null, prefs: HeroPrefs = PREFS) {
	const reload = vi.spyOn(window.location, 'reload').mockImplementation(() => {});
	const onchoose = vi.fn<(next: HeroPrefs) => void>();
	target = document.createElement('div');
	document.body.appendChild(target);
	app = mount(HeroFramingSwitch, { target, props: { prefs, origin, onchoose } });
	const group = target.querySelector('[role="group"]') as HTMLElement;
	const button = (label: string) =>
		Array.from(group.querySelectorAll('button')).find((b) => b.textContent?.trim() === label);
	return { reload, onchoose, group, button };
}

describe('hero framing switch', () => {
	it('is a labelled group of localised choices, the ones in force pressed', () => {
		const { group, button } = mountSwitch('kitsu');
		expect(group.getAttribute('aria-label')).toBe(m.hero_framing_group_label());
		const labels = {
			[m.hero_framing_top()]: 'true',
			[m.hero_framing_contain()]: 'false',
			[m.hero_framing_pillar()]: 'false',
			[m.hero_framing_focus()]: 'false',
			[m.hero_framing_source_kitsu()]: 'true',
			[m.hero_framing_source_anilist()]: 'false'
		};
		for (const [label, pressed] of Object.entries(labels)) {
			const b = button(label);
			expect(b, label).toBeDefined();
			expect(b!.getAttribute('aria-pressed'), label).toBe(pressed);
		}
		expect(group.querySelectorAll('button')).toHaveLength(6);
	});

	it('says which image is on screen', () => {
		const cases: [HeroOrigin | null, string][] = [
			['anilist', m.hero_framing_origin_anilist()],
			['kitsu', m.hero_framing_origin_kitsu()],
			['poster', m.hero_framing_origin_poster()],
			[null, m.hero_framing_origin_none()]
		];
		for (const [origin, text] of cases) {
			const { group } = mountSwitch(origin);
			expect(group.textContent, String(origin)).toContain(m.hero_framing_origin_prefix());
			expect(group.textContent, String(origin)).toContain(text);
			if (app) unmount(app);
			app = null;
			target?.remove();
		}
	});

	it('hands a framing choice to the page, keeping the rest, without a reload', () => {
		const { reload, onchoose, button } = mountSwitch('kitsu');
		button(m.hero_framing_pillar())!.click();
		expect(onchoose).toHaveBeenCalledTimes(1);
		expect(onchoose).toHaveBeenCalledWith({ ...PREFS, framing: 'pillar' });
		expect(reload).not.toHaveBeenCalled();
		// The page owns persistence.
		expect(localStorage.getItem(HERO_PREFS_STORAGE_KEY)).toBeNull();
	});

	it('hands a source choice to the page the same way', () => {
		const { onchoose, button } = mountSwitch('kitsu');
		button(m.hero_framing_source_anilist())!.click();
		expect(onchoose).toHaveBeenCalledWith({ ...PREFS, source: 'anilist' });
	});
});
