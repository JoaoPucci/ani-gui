// No template carries a static `style="…"` attribute.
//
// The renderer's content security policy has no 'unsafe-inline' for
// styles. Svelte parses a template's static markup (attributes
// included) as HTML and clones it, and Chromium reports an inline
// style attribute met that way as a violation — the cloned element
// keeps its style, but the report fails the e2e suite's policy guard,
// and an attribute set any other way is refused outright. A style written
// with an expression in it is applied through the CSSOM
// (`element.style`), which the policy does not govern, so only fully
// static attributes are a problem — give those a class instead.
//
// A syntactic constraint over the source text: a `style="…"` whose
// value holds no `{` in any .svelte file under src/. It does not tell
// comments from markup, and does not see a static style written with
// single quotes or as a `style:` directive with a literal value (the
// directive goes through the CSSOM too, so that one is fine).
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

const SRC = fileURLToPath(new URL('../../src', import.meta.url));

function svelteFiles(dir: string): string[] {
	return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
		const p = join(dir, e.name);
		if (e.isDirectory()) return svelteFiles(p);
		return e.name.endsWith('.svelte') ? [p] : [];
	});
}

describe('static style attributes', () => {
	it('no template carries one', () => {
		const found = svelteFiles(SRC).flatMap((f) =>
			readFileSync(f, 'utf8')
				.split('\n')
				.flatMap((line, i) =>
					/\sstyle="[^"{]*"/.test(line) ? [`${relative(SRC, f)}:${i + 1}: ${line.trim()}`] : []
				)
		);
		expect(found).toEqual([]);
	});
});
