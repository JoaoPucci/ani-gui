import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { savedFields } from './set-entry';
import { providerName } from './busy-save';
import { isRateLimit } from './set-entry-outcome';
import { m } from '$lib/paraglide/messages';
import type { ListStatus } from './types';

describe('savedFields, by property', () => {
	it('is the status and progress when the user set the status, the progress alone otherwise', () => {
		fc.assert(
			fc.property(
				fc.constantFrom<ListStatus>('planning', 'watching', 'paused'),
				fc.boolean(),
				fc.nat(24),
				(status, statusChanged, progress) => {
					const fields = savedFields({ status, statusChanged, progress });
					expect(fields).toEqual(statusChanged ? ['status', 'progress'] : ['progress']);
				}
			)
		);
	});
});

describe('providerName, by property', () => {
	it('is the display name of a tracker with one, and the id of one without', () => {
		fc.assert(
			fc.property(fc.constantFrom('anilist' as const, 'mal' as const, 'inhouse' as const), (p) => {
				const want =
					p === 'anilist'
						? m.account_provider_anilist()
						: p === 'mal'
							? m.account_provider_mal()
							: p;
				expect(providerName(p)).toBe(want);
			})
		);
	});
});

describe('isRateLimit, by property', () => {
	it('is an error carrying status 429, and nothing else', () => {
		fc.assert(
			fc.property(fc.integer({ min: 100, max: 599 }), (status) => {
				expect(isRateLimit(Object.assign(new Error('x'), { status }))).toBe(status === 429);
			})
		);
		fc.assert(
			fc.property(fc.anything(), (value) => {
				fc.pre(!(value instanceof Error));
				expect(isRateLimit(value)).toBe(false);
			})
		);
	});
});
