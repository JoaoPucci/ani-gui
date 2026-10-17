import { describe, expect, it } from 'vitest';
import { m } from '$lib/paraglide/messages';
import { locales } from '$lib/paraglide/runtime';
import { versionLabel } from './version-label';

describe('versionLabel', () => {
	it('names each kind with its own message', () => {
		expect(versionLabel('uncensored')).toBe(m.detail_version_uncensored());
		expect(versionLabel('standard')).toBe(m.detail_version_standard());
	});

	it('names the two kinds and the control apart, translated, in every locale', () => {
		const names = (locale: (typeof locales)[number]) => [
			m.detail_version_group_label({}, { locale }),
			m.detail_version_group_aria_label({}, { locale }),
			m.detail_version_uncensored({}, { locale }),
			m.detail_version_standard({}, { locale })
		];
		const english = names('en');
		for (const locale of locales) {
			const [label, aria, u, s] = names(locale);
			for (const text of [label, aria, u, s]) expect(text.trim()).not.toBe('');
			expect(u).not.toBe(s);
			if (locale === 'en') continue;
			expect(u).not.toBe(english[2]);
			expect(s).not.toBe(english[3]);
		}
	});
});
