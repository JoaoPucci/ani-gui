// Acceptance: a row header's line — title, rule, caption, status and
// actions — is spaced by one gap, taken from the spacing scale, and
// wraps instead of overflowing when a translation runs long.
//
// The first inline mock-up spaced the caption, its separator and the
// new pill by three different means (the eyebrow's gap, a separator's
// margin, the header's gap), and the difference was visible. happy-dom
// has no cascade to measure that with, so this is a syntactic
// constraint on the component's style block, in the shape of
// tests/lint and the hero switch's layout case: every rule that spaces
// an item on the line reads the same custom property, and that
// property is a spacing token.

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, it, expect } from 'vitest';

// Vitest runs from frontend/; read while the file is collected.
const SOURCE = readFileSync(resolve(process.cwd(), 'src/lib/components/Strip.svelte'), 'utf8');
const STYLE = SOURCE.slice(SOURCE.indexOf('<style>'));

function rule(selector: string): string {
	const at = STYLE.indexOf(`${selector} {`);
	expect(at, selector).toBeGreaterThan(-1);
	return STYLE.slice(at, STYLE.indexOf('}', at));
}

describe('a row header’s line', () => {
	it('declares its one gap from the spacing scale', () => {
		expect(rule('.strip-header')).toMatch(/--strip-header-gap: var\(--space-\d\);/);
	});

	it('spaces every item on the line with that gap and nothing else', () => {
		for (const selector of ['.strip-header', '.strip-title', '.strip-meta', '.strip-actions']) {
			const r = rule(selector);
			expect(r, selector).toMatch(/(column-)?gap: var\(--strip-header-gap\);/);
			// A margin between items would be a second, different gap.
			expect(r, selector).not.toMatch(/margin-inline/);
		}
		// The one exception, and deliberate: on a band the actions are
		// pushed to the row's end, and come back beside the title when
		// they wrap below it.
		expect(rule(".strip[data-surface='band'] .strip-actions")).toContain(
			'margin-inline-start: auto;'
		);
		const margins = STYLE.match(/margin-inline(-start|-end)?: [^;]+;/g) ?? [];
		expect(margins.filter((d) => !/: 0;$/.test(d))).toEqual(['margin-inline-start: auto;']);
	});

	it('centres its items on one axis and wraps rather than overflowing', () => {
		for (const selector of ['.strip-header', '.strip-title']) {
			const r = rule(selector);
			expect(r, selector).toContain('align-items: center;');
			expect(r, selector).toContain('flex-wrap: wrap;');
		}
		expect(rule('.strip-header')).not.toContain('align-items: baseline');
	});

	it('reserves the action’s height on every row, so rows line up with or without one', () => {
		expect(rule('.strip-header')).toContain('min-block-size: var(--space-6);');
	});
});

describe('a banded row’s edges', () => {
	// The band is a surface the row sits on; the edge fades are what hide
	// a row's overflow. A fade drawn in the page's colour over a band
	// leaves a dark seam at the band's edge, and a band without fades
	// cuts the last card off flat. Both read one surface colour.
	it('fades into the surface the row sits on, not the page', () => {
		for (const selector of ['.strip-fade-start', '.strip-fade-end']) {
			const r = rule(selector);
			expect(r, selector).toContain('var(--strip-surface)');
			expect(r, selector).not.toContain('var(--ink-000)');
		}
	});

	it('sets that surface on a banded row, and keeps its fades', () => {
		expect(rule(".strip[data-surface='band']")).toMatch(/--strip-surface: /);
		expect(STYLE).not.toMatch(/data-surface='band'\][^{]*strip-fade[^{]*\{[^}]*display: none/);
	});
});

describe('a wrapped status line', () => {
	// When a long translation pushes the status onto the next line, its
	// "·" would open that line and separate nothing. Strip marks the
	// header while the status starts a line of its own
	// ($lib/components/strip-header-wrap), and the separator hides.
	it('hides the separator while the status starts a line of its own', () => {
		expect(rule(".strip-header[data-meta-wraps='true'] .strip-meta-sep")).toContain(
			'display: none;'
		);
		expect(SOURCE).toContain('data-meta-wraps={metaWraps}');
	});

	it('puts the status on a line of its own whenever its separator is hidden', () => {
		// The wrap is decided from natural widths, with the hidden
		// separator counted at its last shown width, so the decision does
		// not undo itself once the separator is gone. Hidden separator and
		// status-on-the-title's-line must never coincide: while marked,
		// the title takes the whole first line and the status starts the
		// next, whatever its width.
		expect(rule(".strip-header[data-meta-wraps='true'] .strip-title")).toContain(
			'flex-basis: 100%;'
		);
		expect(SOURCE).toContain('watchHeaderLines(');
	});
});

describe('a banded row’s wrapped actions', () => {
	// On a band the actions are pushed to the band's end. Once they wrap
	// below the title, that push strands them at the far end of an
	// otherwise empty line, away from the title they belong to; they
	// start their line instead.
	it('drop the push to the band’s end once they wrap', () => {
		expect(SOURCE).toContain('data-actions-wrap={actionsWrap}');
		expect(
			rule(".strip[data-surface='band'] .strip-header[data-actions-wrap='true'] .strip-actions")
		).toContain('margin-inline-start: 0;');
	});
});

describe('the band', () => {
	// An inset, rounded, lighter panel per row is the stacked-card look
	// the design rules keep out. A band is a change of ground across the
	// full width of the rows' column: no radius, no inset, the gutter
	// unchanged so cards line up with a plain row's.
	it('runs the full width of the column, square, with the row’s own gutter', () => {
		const band = rule(".strip[data-surface='band']");
		expect(band).not.toContain('border-radius');
		expect(band).not.toContain('margin-inline');
		expect(band).not.toContain('--strip-pad-inner');
	});
});

describe('a header line in a right-to-left locale', () => {
	it('fades its edges toward the side the row continues on', () => {
		expect(rule('.strip-fade-start:dir(rtl)')).toContain('to left');
		expect(rule('.strip-fade-end:dir(rtl)')).toContain('to right');
	});
});

describe('the status’s voice', () => {
	it('is the caption’s, taken from the eyebrow classes rather than copied', () => {
		expect(SOURCE).toContain('class="eyebrow eyebrow-value strip-meta"');
		const meta = rule('.strip-meta');
		for (const copied of ['font-size', 'letter-spacing', 'text-transform', 'font-family']) {
			expect(meta, copied).not.toContain(copied);
		}
	});
});
