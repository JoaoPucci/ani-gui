// In a forced-colours theme, what the app paints itself is in the
// theme's colours.
//
// A Windows contrast theme (`forced-colors: active`) repaints every
// colour a page sets with its own, so a state shown only by a fill
// would vanish. The app keeps such a state visible by letting the
// element that carries it opt out of that repainting
// (`forced-color-adjust: none`) and drawing it in system colours
// itself. What an element that opts out draws is no longer the
// theme's: an app colour there (an accent, a grey) shows as authored,
// whatever the theme, and may not be readable against it.
//
// So inside every forced-colours block (`@media (forced-colors:
// active)`, or `(forced-colors)`), each `color`, `background`,
// `background-color`, `border-color`, `border-*-color` and
// `outline-color` is written as a single CSS system colour keyword
// (`Highlight`, `CanvasText`, ...) or `transparent` (or `none`, for a
// background); a `border` or `outline` shorthand's colour likewise;
// and a `box-shadow` is `none`, since opting out brings shadows back.
//
// Held by parsers and syntactic constraints, never by reading what a
// value means: postcss reads every stylesheet under `src/` and every
// component's `<style>` (found by Svelte's compiler), and each value's
// words are compared, as text, against the list of keywords (a
// shorthand's lengths and line styles are recognised by their words
// too). A value written as `var(--name, fallback)` is held by its
// fallback's words.
//
// What it does not see: a colour set on an element that opts out from
// a rule outside the block (its resting colour or shadow), which the
// block must override; a custom property's own value, wherever it is
// set; colour properties outside the list (`fill`, `stroke`,
// `caret-color`, `text-decoration-color`, `accent-color`, ...); styles
// set in markup (`style=`, `style:`), from script or in `app.html`; and
// whether a state shows at all, which is the Electron e2e's
// (`electron/e2e/forced-colours-state.spec.ts`).

import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import postcss, { type AtRule } from 'postcss';
import { parse } from 'svelte/compiler';
import { describe, expect, it } from 'vitest';

const FRONTEND = fileURLToPath(new URL('../..', import.meta.url));
const SRC = join(FRONTEND, 'src');
const GENERATED = join(SRC, 'lib', 'paraglide');

const files = (readdirSync(SRC, { recursive: true }) as string[])
	.map((rel) => join(SRC, rel))
	.filter((path) => !path.startsWith(GENERATED));
const components = files.filter((path) => path.endsWith('.svelte'));
const stylesheets = files.filter((path) => path.endsWith('.css'));

/** The CSS system colour keywords (CSS Color 4, plus the two accent
 *  colours), and the two that paint nothing. */
const SYSTEM = new Set(
	[
		'Canvas',
		'CanvasText',
		'LinkText',
		'VisitedText',
		'ActiveText',
		'ButtonFace',
		'ButtonText',
		'ButtonBorder',
		'Field',
		'FieldText',
		'Highlight',
		'HighlightText',
		'SelectedItem',
		'SelectedItemText',
		'Mark',
		'MarkText',
		'GrayText',
		'AccentColor',
		'AccentColorText',
		'transparent'
	].map((k) => k.toLowerCase())
);

const COLOUR_PROPS =
	/^(color|background|background-color|border(-(block|inline|top|right|bottom|left)(-(start|end))?)?-color|outline-color)$/;
const SHORTHANDS = /^(border(-(block|inline|top|right|bottom|left)(-(start|end))?)?|outline)$/;
/** A value written as `var(--name, fallback)`: its fallback's words,
 *  which are what paints where the property is not set. */
const fallbackOf = (value: string) => /^var\(--[\w-]+,\s*(.*)\)$/.exec(value)?.[1] ?? null;

/** The words of a value, as written. */
const words = (value: string) => value.trim().split(/\s+/);

/** Whether a word is a length, a line style or another non-colour part
 *  of a border or outline shorthand. */
const notColour = (word: string) =>
	/^-?[\d.]+[a-z%]*$/i.test(word) ||
	/^(none|hidden|dotted|dashed|solid|double|groove|ridge|inset|outset|auto|thin|medium|thick)$/i.test(
		word
	);

/** The declarations inside forced-colours blocks of `css` that paint in
 *  a colour other than the theme's, as `where: prop: value`. */
function offenders(path: string, text: string, css: string, offset: number): string[] {
	const found: string[] = [];
	postcss.parse(css).walkAtRules('media', (media: AtRule) => {
		if (!forced(media.params)) return;
		media.walkDecls((decl) => {
			// A custom property's fallback is read in its place: the
			// property itself, set elsewhere, is not seen.
			const value = (fallbackOf(decl.value.trim()) ?? decl.value).trim();
			let bad = false;
			if (decl.prop === 'box-shadow') {
				// Opting out of the theme brings shadows back; none is drawn.
				bad = value !== 'none';
			} else if (COLOUR_PROPS.test(decl.prop)) {
				const one = words(value).length === 1 ? value.toLowerCase() : '';
				bad = !(SYSTEM.has(one) || (one === 'none' && decl.prop.startsWith('background')));
			} else if (SHORTHANDS.test(decl.prop)) {
				bad = words(value).some((w) => !notColour(w) && !SYSTEM.has(w.toLowerCase()));
			}
			if (bad) {
				const at = offset + (decl.source?.start?.offset ?? 0);
				const line = text.slice(0, at).split('\n').length;
				found.push(`${relative(SRC, path)}:${line}: ${decl.prop}: ${value}`);
			}
		});
	});
	return found;
}

/** Whether a media query's words name forced colours in force, in any
 *  of its spellings: `(forced-colors: active)`, `(forced-colors)`. A
 *  query that names `none` is the other side. */
const forced = (params: string) =>
	params.replace(/\s+/g, '').includes('forced-colors') &&
	!/forced-colors:none/.test(params.replace(/\s+/g, ''));

/** How many forced-colours blocks `css` holds. */
function blocks(css: string): number {
	let n = 0;
	postcss.parse(css).walkAtRules('media', (media) => {
		if (forced(media.params)) n++;
	});
	return n;
}

function componentStyle(text: string, path: string): { css: string; offset: number } | null {
	const ast = parse(text, { modern: true, filename: path });
	return ast.css ? { css: ast.css.content.styles, offset: ast.css.content.start } : null;
}

const sheets = [
	...stylesheets.map((path) => {
		const text = readFileSync(path, 'utf8');
		return { path, text, css: text, offset: 0 };
	}),
	...components.flatMap((path) => {
		const text = readFileSync(path, 'utf8');
		const style = componentStyle(text, path);
		return style ? [{ path, text, ...style }] : [];
	})
];

describe('forced colours: the theme paints', () => {
	it('found the controls that keep their state in a forced-colours theme', () => {
		// An empty walk must not read as a clean one: a floor on the
		// files that hold a forced-colours block (eleven do today).
		const holding = sheets.filter((s) => blocks(s.css) > 0).map((s) => relative(SRC, s.path));
		expect(holding.length).toBeGreaterThanOrEqual(10);
	});

	it('every colour set in a forced-colours block is a system colour', () => {
		const found = sheets.flatMap((s) => offenders(s.path, s.text, s.css, s.offset));
		expect(found, `an app colour in a forced-colours block:\n${found.join('\n')}`).toEqual([]);
	});

	// The sweep passes on a clean tree, which is also what a check that
	// never fires looks like. These run it over fixtures.
	const sheetOf = (css: string) => offenders(join(SRC, '__fixture.css'), css, css, 0);
	const wrap = (rule: string) => `@media (forced-colors: active) { ${rule} }`;

	it.each([
		['an app colour', '.x { background: var(--accent); }'],
		['a hex colour', '.x { color: #fff; }'],
		['a mixed colour', '.x { background-color: color-mix(in oklab, Highlight 50%, Canvas); }'],
		['a border with an app colour', '.x { border: 1px solid var(--ink-300); }'],
		['an outline colour', '.x { outline-color: red; }'],
		['a side border colour', '.x { border-inline-end-color: #000; }'],
		['a token whose fallback is an app colour', '.x { color: var(--x, #f00); }'],
		['a shadow', '.x { box-shadow: 0 0 0 2px red; }']
	])('fails on %s', (_name, rule) => {
		expect(sheetOf(wrap(rule))).toHaveLength(1);
	});

	it.each([
		['a system colour', '.x { forced-color-adjust: none; background: Highlight; }'],
		['its text', '.x { color: HighlightText; }'],
		['a border in a system colour', '.x { border: 1px solid CanvasText; }'],
		[
			'a token whose fallback is the theme',
			'.x { outline: var(--ring-forced, 2px solid CanvasText); }'
		],
		['nothing painted', '.x { background: none; border-color: transparent; }'],
		['no shadow', '.x { box-shadow: none; }'],
		['the other spelling', '@media (forced-colors) { .x { color: Highlight; } }'],
		['outside a forced-colours block', '.x { background: var(--accent); }']
	])('passes on %s', (_name, rule) => {
		const css =
			_name === 'outside a forced-colours block' || rule.startsWith('@media') ? rule : wrap(rule);
		expect(sheetOf(css)).toEqual([]);
	});
});
