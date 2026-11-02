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
		// Declared once, on the slot, for the button and the room held
		// for it alike.
		const slot = rule('.row-action-slot');
		expect(slot).toContain('--ra-type-size: 0.8125rem;');
		expect(slot).toContain('--ra-tracking: 0.14em;');
		const base = rule('.row-action');
		expect(base).toContain('font-size: var(--ra-type-size);');
		expect(base).toContain('letter-spacing: var(--ra-tracking);');
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
		// `inline-size: 100%` is the reserve only because the slot is
		// the button's containing block.
		expect(rule(ICON)).toContain('position: relative;');
	});

	it('holds exactly the open pill’s width: the room and the pill share every measure', () => {
		// The sizer sets the words as the button does. A type value
		// changed on one side only would clip the open label's last
		// letter, or leave dead room after it.
		const sizer = rule(`${ICON}::after`);
		const base = rule('.row-action');
		for (const prop of [
			'font-family',
			'font-size',
			'font-weight',
			'letter-spacing',
			'text-transform'
		]) {
			const decl = new RegExp(`\\n\\s*${prop}: ([^;]+);`);
			const inButton = decl.exec(base)?.[1];
			expect(inButton, prop).toMatch(/^var\(--ra-/);
			expect(decl.exec(sizer)?.[1], prop).toBe(inButton);
		}
		// Its edge and spacing too: the sizer pads for the open pill's
		// start, icon, gap and end, and both draw the same border, each
		// from the one declaration on the slot.
		expect(sizer).toContain(
			'padding-inline: calc(var(--ra-pad) + var(--ra-icon) + var(--ra-gap)) var(--ra-pad);'
		);
		expect(sizer).toContain('border: var(--ra-border) solid transparent;');
		expect(base).toContain('border: var(--ra-border) solid var(--row-action-border);');
		expect(base).toContain('padding-inline: var(--ra-pad);');
		expect(base).toContain('gap: var(--ra-gap);');
		expect(rule('.row-action-slot')).toContain('--ra-border: 1px;');
		const open = rule(`${ICON} .row-action:is(:hover, :focus-visible)`);
		expect(open).toContain('padding-inline-start: var(--ra-pad);');
		expect(open).not.toMatch(/padding-inline-end|gap:/);
		expect(rule(`${ICON} .row-action`)).toContain('gap: var(--ra-gap);');
		// The icon's width in the reserve is the size the icon is drawn
		// at, from one constant.
		expect(SOURCE).toContain('size={ICON_PX}');
		expect(SOURCE).toContain('style="--ra-icon: {ICON_PX}px"');
		expect(STYLE).not.toMatch(/--ra-icon:/);
	});

	it('opens to its label on hover and on keyboard focus alike', () => {
		const open = rule(`${ICON} .row-action:is(:hover, :focus-visible)`);
		expect(open).toContain('inline-size: 100%;');
		expect(rule(`${ICON} .row-action:is(:hover, :focus-visible) .row-action-label`)).toContain(
			'opacity: 1;'
		);
	});

	it('shows its tone’s colours on keyboard focus, as on hover', () => {
		// Focus is when an icon-shaped action names itself, so a
		// keyboard user sees Clear history's warning before pressing it.
		expect(STYLE).toContain(
			"[data-shape='icon'] .row-action:focus-visible:not([aria-disabled='true']) {"
		);
		expect(STYLE).toContain(
			"[data-shape='icon'] .row-action.quiet:focus-visible:not([aria-disabled='true']) {"
		);
	});

	it('lets its label go before the closing edge reaches the words', () => {
		// At rest the label's transition is the closing one: it fades in
		// half the hover time, and the shape waits that long before it
		// starts to close, so the label is gone before the edge moves.
		expect(rule(`${ICON} .row-action-label`)).toContain(
			'transition: opacity calc(var(--dur-fast) / 2) var(--ease-out-soft);'
		);
		const rest = rule(`${ICON} .row-action`);
		for (const prop of ['inline-size', 'padding-inline-start']) {
			expect(rest, prop).toContain(
				`${prop} var(--dur-med) var(--ease-in-out) calc(var(--dur-fast) / 2)`
			);
		}
		expect(rule(`${ICON} .row-action :global(svg)`)).toContain(
			'transition: scale var(--dur-med) var(--ease-in-out) calc(var(--dur-fast) / 2);'
		);
	});

	it('shrinks its icon to the labelled pill’s size as it opens', () => {
		expect(rule(`${ICON} .row-action :global(svg)`)).toMatch(/scale: 1\.\d+;/);
		expect(rule(`${ICON} .row-action:is(:hover, :focus-visible) :global(svg)`)).toContain(
			'scale: 1;'
		);
	});

	it('never hides its label from a screen reader', () => {
		const rules = [...STYLE.matchAll(/[^{}]*\.row-action-label[^{]*\{[^}]*\}/g)];
		// The rest and the open rule at least: a renamed class must not
		// leave this passing over nothing.
		expect(rules.length).toBeGreaterThanOrEqual(2);
		for (const [at] of rules) {
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
	it('is a clock whose rim gives way to an ×, large enough to read at 14px', () => {
		const icon = readFileSync(resolve(process.cwd(), 'src/lib/components/Icon.svelte'), 'utf8');
		const at = icon.indexOf("name === 'clear-history'");
		expect(at).toBeGreaterThan(-1);
		const glyph = icon.slice(at, icon.indexOf('</svg>', at));
		expect(glyph).toContain('d="M12.94 18.24 A7.5 7.5 0 1 1 18.24 12.94"');
		// 6.5 units: about 3.8px at 14px. At 5 units the × read as a speck.
		expect(glyph).toContain('d="M14.5 14.5 L21 21"');
		expect(glyph).toContain('d="M21 14.5 L14.5 21"');
	});
});
