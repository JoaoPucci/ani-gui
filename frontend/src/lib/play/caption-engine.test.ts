import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import {
	LABEL_MARK,
	distinctLabel,
	engineIdOf,
	enginePlan,
	namesTrack,
	sameListing,
	type EngineTrack
} from './caption-engine';
import { cleanLabel } from './caption-label';

const en: EngineTrack = { name: 'English', lang: 'en' };
const es: EngineTrack = { name: 'Español', lang: 'es' };

describe('namesTrack — whose track hls.js takes a text track for', () => {
	it('takes the same label and language for its rendition, whatever the case', () => {
		expect(namesTrack(en, { label: 'english', language: 'EN' })).toBe(true);
	});

	it('takes a track of no language by its label alone', () => {
		expect(namesTrack(en, { label: 'English', language: '' })).toBe(true);
	});

	it('does not take a track in another language or under another label', () => {
		expect(namesTrack(en, { label: 'English', language: 'fr' })).toBe(false);
		expect(namesTrack(en, { label: 'Anglais', language: 'en' })).toBe(false);
	});
});

describe('engineIdOf', () => {
	it("is the rendition's index, or -1 for none of them", () => {
		expect(engineIdOf({ label: 'Español', language: 'es' }, [en, es])).toBe(1);
		expect(engineIdOf({ label: 'Français', language: 'fr' }, [en, es])).toBe(-1);
	});
});

describe('enginePlan', () => {
	it('selects and displays a rendition picked', () => {
		expect(enginePlan(1, null)).toEqual({ subtitleTrack: 1, subtitleDisplay: true, hidden: null });
	});

	it('selects none for a track that is none of its renditions', () => {
		expect(enginePlan(-1, null)).toEqual({
			subtitleTrack: -1,
			subtitleDisplay: true,
			hidden: null
		});
	});

	it('selects none for Off when no fragment is on its way', () => {
		expect(enginePlan('off', null)).toEqual({
			subtitleTrack: -1,
			subtitleDisplay: true,
			hidden: null
		});
	});

	// A rendition left hidden while the engine selects none, with nothing
	// showing, is read by the engine as its pick, and it would show it.
	it('keeps the rendition whose fragment is on its way selected, its display off, for Off', () => {
		expect(enginePlan('off', 0)).toEqual({ subtitleTrack: 0, subtitleDisplay: false, hidden: 0 });
	});

	// The engine drops the cues of a fragment parsed while its track is
	// disabled, and never fetches the fragment again.
	it('leaves the rendition whose fragment is on its way hidden when another track is picked', () => {
		expect(enginePlan(-1, 0).hidden).toBe(0);
		expect(enginePlan(1, 0).hidden).toBe(0);
	});

	it('hides nothing more when the rendition on its way is the one picked', () => {
		expect(enginePlan(0, 0)).toEqual({ subtitleTrack: 0, subtitleDisplay: true, hidden: null });
	});

	it('for any pick, leaves no fragment on its way to a disabled track, and shows only a rendition picked', () => {
		const want = fc.oneof(fc.constant('off' as const), fc.integer({ min: -1, max: 5 }));
		const inFlight = fc.option(fc.integer({ min: 0, max: 5 }), { nil: null });
		fc.assert(
			fc.property(want, inFlight, (w, f) => {
				const plan = enginePlan(w, f);
				// The rendition on its way is the one shown, or kept hidden.
				if (f !== null) {
					const shown = plan.subtitleDisplay && plan.subtitleTrack === f;
					expect(shown || plan.hidden === f).toBe(true);
				}
				// The engine displays a track only when a rendition is picked.
				if (plan.subtitleTrack >= 0 && plan.subtitleDisplay) expect(plan.subtitleTrack).toBe(w);
				// Off never shows anything.
				if (w === 'off') expect(plan.subtitleTrack === -1 || !plan.subtitleDisplay).toBe(true);
				// A rendition picked is the engine's pick.
				if (typeof w === 'number' && w >= 0) expect(plan.subtitleTrack).toBe(w);
				// Anything else leaves the engine on none, or on the
				// rendition on its way with its display off.
				if (w === -1) expect(plan.subtitleTrack).toBe(-1);
			})
		);
	});
});

describe('distinctLabel', () => {
	it('keeps a label no rendition is named by', () => {
		expect(distinctLabel('Français', 'fr', [en, es])).toBe('Français');
	});

	it('marks a label a rendition is named by, in the same language', () => {
		const label = distinctLabel('English', 'en', [en, es]);
		expect(label).not.toBe('English');
		expect(namesTrack(en, { label, language: 'en' })).toBe(false);
	});

	it('leaves the label as the captions menu reads it', () => {
		expect(cleanLabel(distinctLabel('English', 'en', [en]))).toBe('English');
	});

	it('for any label and renditions, no rendition is named by the result, which starts with the label and reads as it', () => {
		const word = fc.constantFrom(
			'English',
			'english',
			'Español',
			'Deutsch',
			'',
			`English${LABEL_MARK}`
		);
		const lang = fc.constantFrom('', 'en', 'EN', 'es', 'de');
		const rendition = fc.record({ name: word, lang });
		fc.assert(
			fc.property(word, lang, fc.array(rendition, { maxLength: 5 }), (label, language, rs) => {
				const out = distinctLabel(label, language, rs);
				expect(out.startsWith(label)).toBe(true);
				for (const r of rs) expect(namesTrack(r, { label: out, language })).toBe(false);
				if (!rs.some((r) => namesTrack(r, { label, language }))) expect(out).toBe(label);
				expect(cleanLabel(out)).toBe(cleanLabel(label));
			})
		);
	});
});

describe('sameListing', () => {
	it('reads two listings as the same when every rendition reads the same, in order', () => {
		expect(sameListing([en, es], [{ ...en }, { ...es }])).toBe(true);
		expect(sameListing([en, es], [es, en])).toBe(false);
		expect(sameListing([en], [en, es])).toBe(false);
		expect(sameListing([en], [{ name: 'English', lang: 'en-GB' }])).toBe(false);
	});

	it('for any listings, agrees with a rendition-by-rendition comparison', () => {
		const r = fc.record({
			name: fc.constantFrom('English', 'Español', 'Deutsch'),
			lang: fc.constantFrom('en', 'es', 'de')
		});
		fc.assert(
			fc.property(fc.array(r, { maxLength: 4 }), fc.array(r, { maxLength: 4 }), (a, b) => {
				const same =
					a.length === b.length && a.every((x, i) => x.name === b[i].name && x.lang === b[i].lang);
				expect(sameListing(a, b)).toBe(same);
				expect(
					sameListing(
						a,
						a.map((x) => ({ ...x }))
					)
				).toBe(true);
			})
		);
	});
});
