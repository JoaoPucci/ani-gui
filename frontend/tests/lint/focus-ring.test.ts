// A control that draws its own focus outline turns the app's ring off.
//
// Every focused control gets the app's ring: `:focus-visible` in
// `src/lib/design/tokens.css` sets `box-shadow: var(--ring)` on it. A
// control that draws a focus outline of its own and leaves that shadow
// in place shows two rings, the outline over the app's. So a style rule
// whose selector names `:focus-visible` and that draws an outline must
// also set `box-shadow` (to `none`, or to the shadow the control keeps)
// in the same rule.
//
// Held by a parser: postcss reads every stylesheet and every
// component's `<style>` (found by Svelte's compiler); the check reads
// each rule's selector and declarations.
//
// What it does not see, deliberately: a ring drawn by a rule whose
// selector does not name `:focus-visible` (`:focus`, `:focus-within`),
// a shadow the control sets in another rule, and styles set from
// script or an inline style. Whether a control shows its ring once and
// whole on screen is the Electron e2e's (`electron/e2e/focus-rings.spec.ts`).

import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import postcss from 'postcss';
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

/** Whether an `outline` or `outline-style` value draws a line. */
function draws(prop: string, value: string): boolean {
	const v = value.trim().toLowerCase();
	if (prop === 'outline') return !/^(none|0(px)?)(\s|$)/.test(v);
	return v !== 'none';
}

/** The rules in `css` that draw a focus outline and leave the app's
 *  ring in place, as `where: selector`. */
function offenders(path: string, text: string, css: string, offset: number): string[] {
	const found: string[] = [];
	postcss.parse(css).walkRules((rule) => {
		if (!rule.selector.includes(':focus-visible')) return;
		let outline = false;
		let shadow = false;
		rule.each((node) => {
			if (node.type !== 'decl') return;
			if (
				(node.prop === 'outline' || node.prop === 'outline-style') &&
				draws(node.prop, node.value)
			)
				outline = true;
			if (node.prop === 'box-shadow') shadow = true;
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
		]
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
		['no focus at all', '.x:hover { outline: 2px solid red; }']
	])('passes on %s', (_name, css) => {
		expect(sheetOf(css)).toEqual([]);
	});
});
