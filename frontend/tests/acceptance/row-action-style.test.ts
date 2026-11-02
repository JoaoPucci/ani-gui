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

// The inline row's icon-only action. Opening is a cascade change, so it
// too is read as constraints on the style block.
describe('an icon-shaped row action', () => {
	const ICON = ".row-action-slot[data-shape='icon']";

	it('rests as a 32px circle over the width its label will need', () => {
		const rest = rule(`${ICON} .row-action`);
		expect(rest).toContain('position: absolute;');
		expect(rest).toContain('inline-size: var(--space-6);');
		expect(rest).toContain('overflow: hidden;');
		// The slot holds the open width, from the label itself.
		const sizer = rule(`${ICON}::after`);
		expect(sizer).toContain('content: attr(data-label);');
		expect(sizer).toContain('visibility: hidden;');
	});

	it('opens to its label on hover and on keyboard focus alike', () => {
		const open = rule(`${ICON} .row-action:is(:hover, :focus-visible)`);
		expect(open).toContain('inline-size: 100%;');
		expect(rule(`${ICON} .row-action:is(:hover, :focus-visible) .row-action-label`)).toContain(
			'opacity: 1;'
		);
	});

	it('shrinks its icon to the labelled pill’s size as it opens', () => {
		expect(rule(`${ICON} .row-action :global(svg)`)).toMatch(/scale: 1\.\d+;/);
		expect(rule(`${ICON} .row-action:is(:hover, :focus-visible) :global(svg)`)).toContain(
			'scale: 1;'
		);
	});

	it('never hides its label from a screen reader', () => {
		for (const [at] of STYLE.matchAll(/[^{}]*\.row-action-label[^{]*\{[^}]*\}/g)) {
			expect(at).not.toMatch(/visibility:\s*hidden|display:\s*none/);
		}
	});

	it('times every transition from the duration tokens, which reduced motion zeroes', () => {
		const transitions = [...STYLE.matchAll(/transition:([^;]*);/g)].map((m) => m[1]);
		expect(transitions.length).toBeGreaterThan(0);
		for (const t of transitions) expect(t).not.toMatch(/\d(ms|s)\b/);
		const tokens = readFileSync(resolve(process.cwd(), 'src/lib/design/tokens.css'), 'utf8');
		const reduced = tokens.slice(tokens.indexOf('prefers-reduced-motion: reduce'));
		expect(reduced).toContain('--dur-fast: 0ms;');
		expect(reduced).toContain('--dur-med: 0ms;');
	});
});

describe('the clear-history glyph', () => {
	it('is a clock whose rim gives way to an ×', () => {
		const icon = readFileSync(resolve(process.cwd(), 'src/lib/components/Icon.svelte'), 'utf8');
		const at = icon.indexOf("name === 'clear-history'");
		expect(at).toBeGreaterThan(-1);
		const glyph = icon.slice(at, icon.indexOf('</svg>', at));
		expect(glyph).toContain('d="M12.94 18.24 A7.5 7.5 0 1 1 18.24 12.94"');
		expect(glyph).toContain('d="M15.5 15.5 L20.5 20.5"');
	});
});
