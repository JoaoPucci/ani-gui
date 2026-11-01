// Acceptance: a row action is visible as a control and set in its
// row's voice. happy-dom has no cascade to measure, so these are
// syntactic constraints on the component's style block, in the shape
// of the strip header's layout cases.
//
// - Its border is what marks it as a button on a line of uppercase
//   text, so it carries at least 3:1 against the page: 40% bone over
//   ink-000 is about 3.4:1. The quiet tone keeps that border and is
//   told apart by its text colour, not by a fainter edge.
// - Its type is the eyebrow's — 13px, 0.14em tracking — so the line it
//   sits on reads as one voice.

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, it, expect } from 'vitest';

const SOURCE = readFileSync(resolve(process.cwd(), 'src/lib/components/RowAction.svelte'), 'utf8');
const STYLE = SOURCE.slice(SOURCE.indexOf('<style>'));

function rule(selector: string): string {
	const at = STYLE.indexOf(`${selector} {`);
	expect(at, selector).toBeGreaterThan(-1);
	return STYLE.slice(at, STYLE.indexOf('}', at));
}

describe('a row action', () => {
	it('draws its edge at 40% bone, in both tones', () => {
		const base = rule('.row-action');
		expect(base).toContain(
			'--row-action-border: color-mix(in oklab, var(--bone-100) 40%, transparent);'
		);
		expect(base).toContain('border: 1px solid var(--row-action-border);');
		expect(rule('.row-action.quiet')).not.toContain('border-color');
	});

	it('is set in the eyebrow’s type', () => {
		const base = rule('.row-action');
		expect(base).toContain('font-size: 0.8125rem;');
		expect(base).toContain('letter-spacing: 0.14em;');
	});
});

describe('the refresh glyph', () => {
	// The arc ended short of the arrowhead's corner, which shows at any
	// size above 14px. The arc ends where the head turns.
	it('closes its arc on its arrowhead', () => {
		const icon = readFileSync(resolve(process.cwd(), 'src/lib/components/Icon.svelte'), 'utf8');
		const at = icon.indexOf("name === 'refresh'");
		const glyph = icon.slice(at, icon.indexOf('</svg>', at));
		expect(glyph).toContain('d="M19.5 12 A7.5 7.5 0 1 1 18 7.5"');
		expect(glyph).toContain('d="M18 3.5 V7.5 H14"');
	});
});
