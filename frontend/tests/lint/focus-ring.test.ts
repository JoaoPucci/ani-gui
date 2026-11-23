// A control that draws its own focus outline turns the app's ring off.
//
// Every focused control gets the app's ring: `:focus-visible` in
// `src/lib/design/tokens.css` sets `box-shadow: var(--ring)` on it. A
// control that draws a focus outline of its own and leaves that shadow
// in place can show two rings, the outline over the app's. So a style
// rule that styles a focused element (`:focus-visible` in the last
// compound of its selector, directly or inside `:is()`, `:where()` or
// Svelte's `:global()`) and draws an outline must also set `box-shadow`
// in the same rule, to a value that does not name the app's ring (its
// text does not contain `--ring`).
//
// The rule is conservative: a control whose own resting rule sets a
// shadow already outranks the app's ring, and is held to it anyway, so
// a shadow added under the mouse cannot bring a second ring back.
//
// Held by parsers and syntactic constraints, never by reading what a
// value means: postcss reads every stylesheet under `src/` and every
// component's `<style>` (found by Svelte's compiler), and Svelte's CSS
// parser reads each selector. Declaration values are compared as
// text: an `outline` or `outline-style` written as anything but
// `none` or `0` counts as drawing an outline, whatever it would draw
// (a `transparent` outline, one with no line style, `inherit`), and a
// `box-shadow` counts unless its text contains `--ring`.
//
// What it does not see, deliberately:
//
// - A ring drawn by a rule whose selector does not put `:focus-visible`
//   on the element it styles (`:focus`, `:focus-within`, a descendant
//   of a focused element), a shadow the control sets in another rule,
//   and styles set from script or an inline style.
// - A shadow that sets the app's ring again through another custom
//   property, or by writing its value out.
//
// Whether a control shows its ring once and whole on screen is the
// Electron e2e's (`electron/e2e/focus-rings.spec.ts`).

import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import postcss from 'postcss';
import { parse, parseCss } from 'svelte/compiler';
import { describe, expect, it } from 'vitest';

const FRONTEND = fileURLToPath(new URL('../..', import.meta.url));
const SRC = join(FRONTEND, 'src');
const GENERATED = join(SRC, 'lib', 'paraglide');

const files = (readdirSync(SRC, { recursive: true }) as string[])
	.map((rel) => join(SRC, rel))
	.filter((path) => !path.startsWith(GENERATED));
const components = files.filter((path) => path.endsWith('.svelte'));
const stylesheets = files.filter((path) => path.endsWith('.css'));

/** The two values that turn an outline off, as written. Anything else
 *  written for `outline` or `outline-style` counts as drawing one: the
 *  check compares the value's text, it does not read what it means. */
const OFF = new Set(['none', '0']);

type Simple = { type: string; name?: string; args?: { children: Complex[] } | null };
type Complex = { children: { selectors: Simple[] }[] };

/** Whether a compound selector puts `:focus-visible` on its element. */
function focusedCompound(selectors: Simple[]): boolean {
	return selectors.some(
		(s) =>
			s.type === 'PseudoClassSelector' &&
			(s.name === 'focus-visible' ||
				((s.name === 'is' || s.name === 'where' || s.name === 'global') &&
					(s.args?.children ?? []).some(styledFocused)))
	);
}

/** Whether a complex selector styles a focused element: its last
 *  compound names `:focus-visible`. */
function styledFocused(complex: Complex): boolean {
	const last = complex.children.at(-1);
	return last ? focusedCompound(last.selectors) : false;
}

/** Whether any selector in a rule's list styles a focused element. */
function stylesFocused(selector: string): boolean {
	const sheet = parseCss(`${selector} {}`) as unknown as {
		children: { prelude: { children: Complex[] } }[];
	};
	return sheet.children.some((rule) => rule.prelude.children.some(styledFocused));
}

/** The rules in `css` that draw a focus outline and leave the app's
 *  ring in place, as `where: selector`. */
function offenders(path: string, text: string, css: string, offset: number): string[] {
	const found: string[] = [];
	postcss.parse(css).walkRules((rule) => {
		if (!rule.selector.includes(':focus-visible') || !stylesFocused(rule.selector)) return;
		let outline = false;
		let shadow = false;
		rule.each((node) => {
			if (node.type !== 'decl') return;
			if ((node.prop === 'outline' || node.prop === 'outline-style') && !OFF.has(node.value.trim()))
				outline = true;
			if (node.prop === 'box-shadow' && !node.value.includes('--ring')) shadow = true;
		});
		if (outline && !shadow) {
			const at = offset + (rule.source?.start?.offset ?? 0);
			const line = text.slice(0, at).split('\n').length;
			found.push(`${relative(SRC, path)}:${line}: ${rule.selector.replace(/\s+/g, ' ')}`);
		}
	});
	return found;
}

function componentStyle(text: string, path: string): { css: string; offset: number } | null {
	const ast = parse(text, { modern: true, filename: path });
	return ast.css ? { css: ast.css.content.styles, offset: ast.css.content.start } : null;
}

describe('focus rings: one per control', () => {
	it('found the components and the stylesheets', () => {
		// An empty walk must not read as a clean one.
		expect(components.length).toBeGreaterThan(20);
		expect(stylesheets.length).toBeGreaterThan(0);
	});

	it('every rule that draws a focus outline turns the app ring off', () => {
		const found = [
			...stylesheets.flatMap((path) => {
				const text = readFileSync(path, 'utf8');
				return offenders(path, text, text, 0);
			}),
			...components.flatMap((path) => {
				const text = readFileSync(path, 'utf8');
				const style = componentStyle(text, path);
				return style ? offenders(path, text, style.css, style.offset) : [];
			})
		];
		expect(found, `a focus outline over the app's ring:\n${found.join('\n')}`).toEqual([]);
	});

	// The sweep passes on a clean tree, which is also what a check that
	// never fires looks like. These run it over fixtures.
	const sheetOf = (css: string) => offenders(join(SRC, '__fixture.css'), css, css, 0);

	it.each([
		['an outline', '.x:focus-visible { outline: 2px solid red; }'],
		['an outline style', '.x:focus-visible { outline-style: solid; }'],
		['one of a list', '.x:hover, .y:focus-visible { outline: 2px solid red; }'],
		[
			'inside a media query',
			'@media (min-width: 1px) { .x:focus-visible { outline: 1px dotted; } }'
		],
		['focus inside :is()', '.x:is(:hover, :focus-visible) { outline: 2px solid red; }'],
		['a nested rule', '.x { &:focus-visible { outline: 2px solid red; } }'],
		['a global selector', ':global(.x:focus-visible) { outline: 2px solid red; }'],
		[
			"the app's ring set again",
			'.x:focus-visible { outline: 2px solid red; box-shadow: var(--ring); }'
		],
		['a custom outline', '.x:focus-visible { outline: var(--focus-line); }'],
		['an escaped line style', '.x:focus-visible { outline: 2px so\\6cid red; }'],
		['an outline with no line style', '.x:focus-visible { outline: 2px red; }'],
		['a transparent outline', '.x:focus-visible { outline: 2px solid transparent; }'],
		['an inherited outline', '.x:focus-visible { outline: inherit; }']
	])('fails on %s with the ring left in place', (_name, css) => {
		expect(sheetOf(css)).toHaveLength(1);
	});

	it.each([
		['the ring turned off', '.x:focus-visible { outline: 2px solid red; box-shadow: none; }'],
		[
			'a shadow of its own',
			'.x:focus-visible { outline: 2px solid red; box-shadow: 0 0 4px red; }'
		],
		['no outline', '.x:focus-visible { outline: none; }'],
		['a zero outline', '.x:focus-visible { outline: 0; }'],
		['no focus at all', '.x:hover { outline: 2px solid red; }'],
		['focus on another element', '.x:not(:focus-visible) { outline: 2px solid red; }'],
		['a focused ancestor', '.x:focus-visible .icon { outline: 2px solid red; }'],
		['a focused child', '.x:has(:focus-visible) { outline: 2px solid red; }']
	])('passes on %s', (_name, css) => {
		expect(sheetOf(css)).toEqual([]);
	});

	it("names a component's rule by the line it is on", () => {
		const text = [
			'<button class="x">x</button>',
			'',
			'<style>',
			'\t.x {',
			'\t\tcolor: red;',
			'\t}',
			'\t.x:focus-visible {',
			'\t\toutline: 2px solid red;',
			'\t}',
			'</style>',
			''
		].join('\n');
		const path = join(SRC, 'routes', '__fixture.svelte');
		const style = componentStyle(text, path);
		expect(style).not.toBeNull();
		expect(offenders(path, text, style!.css, style!.offset)).toEqual([
			'routes/__fixture.svelte:7: .x:focus-visible'
		]);
	});
});
