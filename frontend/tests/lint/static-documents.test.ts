// The files served as they are carry no document that could run script.
//
// The renderer's script rule is in the page's <meta> (SvelteKit's
// kit.csp); the header the shell adds allows inline script only so its
// default-src does not reach scripts, trusting the meta to narrow it.
// A document served without that meta — an HTML file, or an SVG opened
// on its own — would run its inline script under the header alone.
// The SvelteKit bundle's only HTML is the generated index.html, which
// carries the meta. Files under static/ are copied as they are.
//
// A syntactic constraint over static/: no .html, .htm or .xhtml file,
// and no .svg containing `<script` or an `on…=` event attribute. What it
// does not know: it reads only static/, not files a build step adds.
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

const STATIC = fileURLToPath(new URL('../../static', import.meta.url));

function files(dir: string): string[] {
	return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
		const p = join(dir, e.name);
		return e.isDirectory() ? files(p) : [p];
	});
}

describe('static documents', () => {
	it('static/ holds no HTML document', () => {
		expect(files(STATIC).filter((f) => /\.(html?|xhtml)$/i.test(f))).toEqual([]);
	});

	it('no SVG under static/ carries script', () => {
		const scripted = files(STATIC)
			.filter((f) => /\.svg$/i.test(f))
			.filter((f) => /<script|\son[a-z]+\s*=/i.test(readFileSync(f, 'utf8')))
			.map((f) => relative(STATIC, f));
		expect(scripted).toEqual([]);
	});
});
