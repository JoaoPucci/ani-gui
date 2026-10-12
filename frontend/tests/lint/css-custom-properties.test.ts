// Every custom property the styles read is declared somewhere.
//
// A `var(--name)` with nothing behind it does not fail anywhere: the
// declaration that reads it is dropped at computed-value time and the
// element quietly takes the inherited or initial value. That is how
// the search page's filter chips lost their border (`--ink-400`) and
// their size (`--type-body-s`) without a test, a lint or a console
// line noticing. A fallback (`var(--name, #b11a1a)`) hides it further:
// the element renders, in a colour no token defines.
//
// The check is a syntactic constraint over the source text, not a
// reading of it. It collects every `var(--name` in a .svelte, .css or
// .html file under src/, and requires that `--name:` (a declaration,
// in a stylesheet or an inline `style` attribute) or `style:--name`
// (Svelte's style directive) appears somewhere under src/. It knows
// less than that sentence may suggest:
//
// - It does not scope. A property declared on one component passes
//   for a read in another the declaration never reaches.
// - It does not tell comments from code. A comment that writes
//   `var(--old-name)` fails it; a comment that writes `--name:`
//   satisfies it.
// - It does not see properties set from script (`setProperty`), and
//   none are today; one added that way has to be declared in CSS too,
//   or this check has to learn the form.
//
// A `--` inside a name (`.btn--primary:hover`) is not read as a
// declaration: the property has to start the name.
//
// It is here to catch a name nobody defined, which is the failure that
// has happened.
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

const SRC = fileURLToPath(new URL('../../src', import.meta.url));
const GENERATED = join(SRC, 'lib', 'paraglide');

const files = (readdirSync(SRC, { recursive: true }) as string[])
	.map((rel) => join(SRC, rel))
	.filter((path) => !path.startsWith(GENERATED))
	.filter((path) => /\.(svelte|css|html|ts)$/.test(path));

const styleFiles = files.filter((path) => /\.(svelte|css|html)$/.test(path));

const declared = new Set<string>();
for (const path of files) {
	const text = readFileSync(path, 'utf8');
	for (const match of text.matchAll(/(?<![\w-])(--[\w-]+)\s*:/g)) declared.add(match[1]);
	for (const match of text.matchAll(/style:(--[\w-]+)/g)) declared.add(match[1]);
}

describe('custom properties', () => {
	it('the sweep found the stylesheets and the tokens', () => {
		// An empty walk must not read as a clean one.
		expect(styleFiles.some((path) => path.endsWith('tokens.css'))).toBe(true);
		expect(styleFiles.length).toBeGreaterThan(20);
		expect(declared.has('--ink-000')).toBe(true);
	});

	it('every var() names a declared property', () => {
		const undeclared: string[] = [];
		for (const path of styleFiles) {
			const lines = readFileSync(path, 'utf8').split('\n');
			lines.forEach((line, index) => {
				for (const match of line.matchAll(/var\(\s*(--[\w-]+)/g)) {
					if (!declared.has(match[1])) {
						undeclared.push(`${path.slice(SRC.length + 1)}:${index + 1} ${match[1]}`);
					}
				}
			});
		}
		expect(undeclared, `undeclared custom properties:\n${undeclared.join('\n')}`).toEqual([]);
	});
});
