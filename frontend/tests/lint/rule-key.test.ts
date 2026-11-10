import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { ruleKey } from './rule-key';

describe('ruleKey', () => {
	it('joins the path and the selector, collapsing whitespace', () => {
		expect(ruleKey('routes/+layout.svelte', '.nav li.small\n\t\t.nav-link')).toBe(
			'routes/+layout.svelte .nav li.small .nav-link'
		);
	});

	it('writes a Windows path with forward slashes, as the lists are written', () => {
		// `path.relative` returns backslashes on Windows; the exemption
		// lists are written with forward slashes and must match there too.
		expect(ruleKey('routes\\play\\[id]\\+page.svelte', '.ep-card-recheck')).toBe(
			'routes/play/[id]/+page.svelte .ep-card-recheck'
		);
	});

	it('gives the same key for a path whichever separator it uses', () => {
		const segment = fc.stringMatching(/^[\w.+[\]-]{1,12}$/);
		fc.assert(
			fc.property(fc.array(segment, { minLength: 1, maxLength: 5 }), (parts) => {
				expect(ruleKey(parts.join('\\'), '.x')).toBe(ruleKey(parts.join('/'), '.x'));
			})
		);
	});
});
