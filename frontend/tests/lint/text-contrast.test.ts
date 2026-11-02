// Text written in the design tokens clears WCAG AA on every surface the
// tokens define.
//
// The app's small text comes in tiers: bone-100 for primary, then
// bone-200, bone-300 and bone-400 for secondary, tertiary and the
// faintest captions and placeholders, and in places bone-100 mixed
// with `transparent` to a percentage. Each tier can land on any of the
// ink surfaces — the page, a card, an elevated panel, a hover or a
// dropdown row — so each has to clear 4.5:1, the AA minimum for normal
// text, against each of them. The faintest tier is where it failed:
// the poster-card meta (bone-100 at 45%) measured 4.0:1 on the page,
// and bone-400 measured 4.46:1 on ink-100 and 3.84:1 on ink-200.
//
// Two halves, both read with postcss, and component styles found by
// Svelte's compiler, not by reading source text:
//
// - tokens.css. Its `:root` rule is parsed and every `--bone-*` and
//   `--ink-*` it declares is read as a hex colour. Every bone token is
//   a text tier and every ink token but the ones in NOT_A_SURFACE is a
//   surface; the check measures every pair. A tier or a surface added
//   later is measured without touching this file.
//
// - Component styles. Every `color:` declaration in a component's
//   `<style>` (and in any stylesheet) whose value is exactly
//   `color-mix(in oklab, var(--bone-N) P%, transparent)` — a bone tier
//   drawn at alpha P — is composited over every surface and measured
//   the same way. A declaration below AA must be listed in EXEMPT with
//   the reason WCAG excuses it, and every entry there must still match
//   a declaration that is still below AA, so the list cannot rot.
//
// What it does not see, deliberately:
//
// - `opacity` on an element that holds text. Whether a rule's opacity
//   dims text, an image or a decoration is a question about the markup
//   the selector reaches, which no syntactic rule answers; review holds
//   it. The rail's small links and the dimmed episode tiles were fixed
//   by hand for that reason.
// - Colours that are not bone tiers: the accents, the brand colour,
//   white with an alpha on the player's own chrome, and any mix other
//   than the one form above. They are read by review, not here.
// - Text over imagery (a thumbnail's frame number, the hero), whose
//   background is whatever the image is.
// - Surfaces that are not ink tokens, such as the play page's accent
//   glow. Its strongest stop sits behind the player, not behind text.
// - Large text, which needs only 3:1. Every tier is held to 4.5:1, so
//   large text in these tiers passes a fortiori.
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import postcss, { type Rule } from 'postcss';
import { parse } from 'svelte/compiler';
import { describe, expect, it } from 'vitest';

import { AA_TEXT, parseHex, textContrast, type Rgb } from './contrast';

const SRC = fileURLToPath(new URL('../../src', import.meta.url));
const TOKENS = join(SRC, 'lib', 'design', 'tokens.css');
const GENERATED = join(SRC, 'lib', 'paraglide');

/** Ink tokens that are not surfaces text is set on: borders and disabled strokes. */
const NOT_A_SURFACE = new Set(['--ink-300']);

/**
 * Translucent bone text below AA that WCAG 1.4.3 excuses, keyed by
 * file and selector. Inactive controls and pure decoration (a
 * separator glyph that carries no information) are exempt from the
 * criterion; nothing else belongs here.
 */
const EXEMPT: Record<string, string> = {
	'routes/anime/[id]/+page.svelte .ep-jump-go:disabled': 'disabled control',
	'routes/play/[id]/+page.svelte .ep-jump-go:disabled': 'disabled control',
	'routes/play/[id]/+page.svelte .ep-card-foot-dot': 'separator dot between number and length',
	'routes/play/[id]/+page.svelte .np-em-dash': 'em dash between episode number and title',
	'routes/search/+page.svelte .eyebrow-sep': 'separator dot in the results eyebrow'
};

// ---- tokens.css --------------------------------------------------------------

const tokens = new Map<string, Rgb>();
postcss.parse(readFileSync(TOKENS, 'utf8')).walkRules((rule) => {
	if (rule.selector !== ':root' || rule.parent?.type !== 'root') return;
	rule.walkDecls(/^--(bone|ink)-/, (decl) => {
		tokens.set(decl.prop, parseHex(decl.value.trim()));
	});
});

const textTiers = [...tokens.keys()].filter((name) => name.startsWith('--bone-'));
const surfaces = [...tokens.keys()].filter(
	(name) => name.startsWith('--ink-') && !NOT_A_SURFACE.has(name)
);

/** `tier` at `alpha` against every surface, worst first. */
function worstSurface(tier: string, alpha: number): { surface: string; ratio: number } {
	return surfaces
		.map((surface) => ({
			surface,
			ratio: textContrast(tokens.get(tier)!, alpha, tokens.get(surface)!)
		}))
		.sort((a, b) => a.ratio - b.ratio)[0];
}

// ---- component styles ----------------------------------------------------------

const files = (readdirSync(SRC, { recursive: true }) as string[])
	.map((rel) => join(SRC, rel))
	.filter((path) => !path.startsWith(GENERATED));

function styleOf(path: string): string | null {
	const text = readFileSync(path, 'utf8');
	if (path.endsWith('.css')) return text;
	const ast = parse(text, { modern: true, filename: path });
	return ast.css ? ast.css.content.styles : null;
}

/** The one value form measured here: a bone tier mixed with transparent. */
const TRANSLUCENT_BONE =
	/^color-mix\(\s*in\s+oklab\s*,\s*var\(\s*(--bone-\d+)\s*\)\s+(\d+(?:\.\d+)?)%\s*,\s*transparent\s*\)$/;

interface TranslucentText {
	key: string;
	tier: string;
	alpha: number;
}

const translucent: TranslucentText[] = [];
for (const path of files.filter((p) => /\.(svelte|css)$/.test(p))) {
	const css = styleOf(path);
	if (!css) continue;
	postcss.parse(css).walkDecls('color', (decl) => {
		const match = TRANSLUCENT_BONE.exec(decl.value.trim());
		if (!match) return;
		const rule = decl.parent as Rule;
		translucent.push({
			key: `${relative(SRC, path)} ${rule.selector.replace(/\s+/g, ' ')}`,
			tier: match[1],
			alpha: Number(match[2]) / 100
		});
	});
}

describe('text contrast', () => {
	it('read the tokens and the component styles', () => {
		// An empty read must not pass as a clean one.
		expect(textTiers).toEqual(expect.arrayContaining(['--bone-100', '--bone-400']));
		expect(surfaces).toEqual(expect.arrayContaining(['--ink-000', '--ink-200']));
		expect(translucent.length).toBeGreaterThan(10);
	});

	it('every text tier clears AA on every surface', () => {
		const failures = textTiers.flatMap((tier) =>
			surfaces
				.map((surface) => ({
					tier,
					surface,
					ratio: textContrast(tokens.get(tier)!, 1, tokens.get(surface)!)
				}))
				.filter(({ ratio }) => ratio < AA_TEXT)
				.map(({ tier, surface, ratio }) => `${tier} on ${surface}: ${ratio.toFixed(2)}:1`)
		);
		expect(failures, `below ${AA_TEXT}:1`).toEqual([]);
	});

	it('every translucent bone text colour clears AA on every surface, or is exempt', () => {
		const failures = translucent
			.filter(({ key }) => !(key in EXEMPT))
			.map(({ key, tier, alpha }) => ({ key, tier, alpha, ...worstSurface(tier, alpha) }))
			.filter(({ ratio }) => ratio < AA_TEXT)
			.map(
				({ key, tier, alpha, surface, ratio }) =>
					`${key}: ${tier} at ${Math.round(alpha * 100)}% on ${surface} is ${ratio.toFixed(2)}:1`
			);
		expect(failures, `below ${AA_TEXT}:1`).toEqual([]);
	});

	it('every exemption still names a declaration below AA', () => {
		const stale = Object.keys(EXEMPT).filter((key) => {
			const hits = translucent.filter((t) => t.key === key);
			return hits.length === 0 || hits.every((t) => worstSurface(t.tier, t.alpha).ratio >= AA_TEXT);
		});
		expect(stale, 'exemptions with nothing left to excuse').toEqual([]);
	});
});
