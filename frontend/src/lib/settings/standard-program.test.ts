// Settings' lookup of where the External player type's program is
// installed, and what the hint and placeholder say with its answer.

import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { ExternalPlayerKind, StandardProgram } from '$lib/api';
import {
	createStandardLookup,
	standardHint,
	standardPlaceholder,
	type Found
} from './standard-program';

/** A backend whose answers the test releases one by one. */
function backend() {
	const pending: Array<{
		kind: ExternalPlayerKind;
		answer: (a: StandardProgram) => void;
		fail: () => void;
	}> = [];
	const ask = (kind: ExternalPlayerKind) =>
		new Promise<StandardProgram>((answer, reject) => {
			pending.push({ kind, answer, fail: () => reject(new Error('down')) });
		});
	return { ask, pending };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

const vlc = 'C:\\Program Files\\VideoLAN\\VLC\\vlc.exe';

describe('the standard-program lookup', () => {
	it('shows the answer for the type it was asked about', async () => {
		const { ask, pending } = backend();
		const shown: Array<Found | null> = [];
		const lookup = createStandardLookup(ask, (f) => shown.push(f));
		lookup('vlc');
		expect(pending.map((p) => p.kind)).toEqual(['vlc']);
		pending[0].answer({ name: 'vlc', path: vlc, supported: true });
		await flush();
		expect(shown).toEqual([{ kind: 'vlc', path: vlc, supported: true }]);
	});

	it('drops an answer that arrives after the type changed', async () => {
		const { ask, pending } = backend();
		const shown: Array<Found | null> = [];
		const lookup = createStandardLookup(ask, (f) => shown.push(f));
		lookup('vlc');
		lookup('mpv');
		pending[0].answer({ name: 'vlc', path: vlc, supported: true });
		pending[1].answer({ name: 'mpv', path: null, supported: true });
		await flush();
		expect(shown).toEqual([{ kind: 'mpv', path: null, supported: true }]);
	});

	it("drops a stale failure, and shows the current type's failure as no answer", async () => {
		const { ask, pending } = backend();
		const shown: Array<Found | null> = [];
		const lookup = createStandardLookup(ask, (f) => shown.push(f));
		lookup('vlc');
		lookup('mpv');
		pending[0].fail();
		await flush();
		expect(shown).toEqual([]);
		pending[1].fail();
		await flush();
		expect(shown).toEqual([null]);
	});

	it('never asks about Custom, and a pending answer is dropped once Custom is picked', async () => {
		const { ask, pending } = backend();
		const shown: Array<Found | null> = [];
		const lookup = createStandardLookup(ask, (f) => shown.push(f));
		lookup('iina');
		lookup('custom');
		lookup(undefined);
		expect(pending.map((p) => p.kind)).toEqual(['iina']);
		pending[0].answer({ name: 'iina', path: null, supported: false });
		await flush();
		expect(shown).toEqual([]);
	});

	it('reads an answer without `supported` as supported', async () => {
		const { ask, pending } = backend();
		const shown: Array<Found | null> = [];
		const lookup = createStandardLookup(ask, (f) => shown.push(f));
		lookup('mpv');
		pending[0].answer({ name: 'mpv', path: '/usr/bin/mpv' } as StandardProgram);
		await flush();
		expect(shown).toEqual([{ kind: 'mpv', path: '/usr/bin/mpv', supported: true }]);
	});

	it('shows each outcome exactly when it is for the type current when it arrived', async () => {
		const kind = fc.constantFrom<ExternalPlayerKind>('mpv', 'vlc', 'iina', 'custom');
		await fc.assert(
			fc.asyncProperty(
				fc.array(kind, { minLength: 1, maxLength: 6 }),
				fc.array(fc.tuple(fc.nat(), fc.boolean()), { maxLength: 12 }),
				async (asked, releases) => {
					const { ask, pending } = backend();
					let current: ExternalPlayerKind | null = null;
					const seen: Array<{ shown: Found | null; current: ExternalPlayerKind | null }> = [];
					const lookup = createStandardLookup(ask, (f) => seen.push({ shown: f, current }));
					for (const k of asked) {
						current = k;
						lookup(k);
					}
					// Release the answers in an arbitrary order, some failing.
					const open = [...pending.keys()];
					for (const [pick, ok] of releases) {
						if (open.length === 0) break;
						const i = open.splice(pick % open.length, 1)[0];
						const before = seen.length;
						if (ok) pending[i].answer({ name: pending[i].kind, path: null, supported: true });
						else pending[i].fail();
						await flush();
						// Shown exactly when the request is for the current
						// type: an answer as that type's, a failure as none.
						// A stale answer or failure shows nothing.
						if (pending[i].kind === current) {
							expect(seen.length).toBe(before + 1);
							const last = seen[seen.length - 1].shown;
							if (ok) expect(last?.kind).toBe(current);
							else expect(last).toBeNull();
						} else {
							expect(seen.length).toBe(before);
						}
					}
					for (const { shown, current: at } of seen) {
						expect(at).not.toBe('custom');
						if (shown) expect(shown.kind).toBe(at);
					}
					// Custom is never asked about.
					expect(pending.every((p) => p.kind !== 'custom')).toBe(true);
				}
			),
			{ numRuns: 200 }
		);
	});
});

describe('the hint and placeholder', () => {
	const found = (kind: ExternalPlayerKind, path: string | null, supported = true): Found => ({
		kind,
		path,
		supported
	});

	it('says Custom has no standard program', () => {
		expect(standardHint('custom', null)).toEqual({ variant: 'custom' });
		expect(standardPlaceholder('custom', null)).toBe('');
	});

	it('names where the program was found, and uses that path as the placeholder', () => {
		expect(standardHint('vlc', found('vlc', vlc))).toEqual({ variant: 'found', path: vlc });
		expect(standardPlaceholder('vlc', found('vlc', vlc))).toBe(vlc);
	});

	it("says the program wasn't found, with its name as the placeholder", () => {
		expect(standardHint('mpv', found('mpv', null))).toEqual({ variant: 'missing' });
		expect(standardPlaceholder('mpv', found('mpv', null))).toBe('mpv');
	});

	it('says a program does not exist on this OS', () => {
		expect(standardHint('iina', found('iina', null, false))).toEqual({ variant: 'unsupported' });
		expect(standardPlaceholder('iina', found('iina', null, false))).toBe('iina');
	});

	it('names the program alone with no answer, or an answer for another type', () => {
		expect(standardHint('vlc', null)).toEqual({ variant: 'unknown', program: 'vlc' });
		expect(standardHint('iina', found('vlc', vlc))).toEqual({
			variant: 'unknown',
			program: 'iina'
		});
		expect(standardPlaceholder('iina', found('vlc', vlc))).toBe('iina');
	});

	it("never shows another type's answer", () => {
		const kind = fc.constantFrom<ExternalPlayerKind>('mpv', 'vlc', 'iina');
		fc.assert(
			fc.property(kind, kind, fc.option(fc.string()), fc.boolean(), (shownFor, ofType, path, s) => {
				fc.pre(shownFor !== ofType);
				const answer = found(ofType, path, s);
				expect(standardHint(shownFor, answer)).toEqual({ variant: 'unknown', program: shownFor });
				expect(standardPlaceholder(shownFor, answer)).toBe(shownFor);
			})
		);
	});
});
