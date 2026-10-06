// What the CRAP gate's TypeScript complexity counts. The cases that
// matter most are the ones lizard's tokenizer lost its place on: each
// of them dropped functions, or decisions, from the totals without a
// word.
//
// These run in the CRAP job, which installs the frontend's
// dependencies — the tool reads the compiler from them — rather than
// with tests/tools/, whose runner has no node_modules.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { measure } from '../../tools/ts-ccn.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const tool = path.join(repoRoot, 'tools/ts-ccn.mjs');

const units = (source) => measure('probe.ts', source).units.map((u) => [u.name, u.ccn]);

test('a typed function followed by another keeps both', () => {
	// lizard folded `b` — and everything after it — into `a`.
	const src = `function a(): number { return 1; }
function b(x: boolean): number { if (x) { return 1; } return 2; }
function c(): string | null { try { return 'a'; } catch { return null; } }`;
	assert.deepEqual(units(src), [
		['a', 1],
		['b', 2],
		['c', 2]
	]);
});

test('the count is lizard’s: if, for, while, catch, case, a conditional, && and ||', () => {
	const src = `function all(xs: number[], o?: { a?: number }): number {
	if (xs.length === 0 && o === undefined) return 0;
	else if (xs.length > 9 || xs.length === 1) return 1;
	for (const x of xs) {}
	while (false) {}
	do {} while (false);
	try {} catch (e) {}
	switch (xs[0]) { case 1: break; case 2: break; default: break; }
	return xs.length > 2 ? 1 : 0;
}`;
	// 1 + if + && + if + || + for + while + while + catch + case + case + ?
	assert.deepEqual(units(src), [['all', 12]]);
});

test('logical assignments count as their operators do', () => {
	const src = 'function f(a: { x?: number; y?: boolean }): void { a.x ||= 1; a.y &&= false; }';
	assert.deepEqual(units(src), [['f', 3]]);
});

test('decisions outside any function are reported, not counted', () => {
	const src = "const ready = typeof window !== 'undefined' ? 1 : 0;\nif (ready) {}\nexport function f(): void {}";
	const { units: measured, outside } = measure('probe.ts', src);
	assert.deepEqual(
		measured.map((u) => [u.name, u.ccn]),
		[['f', 1]]
	);
	assert.equal(outside, 2);
});

test('optional markers, ?. and ?? are not conditionals, and text is text', () => {
	const src = 'function f(a?: string, o?: { b?: number }): string {\n' + '\treturn `${a ?? "if"} && ${o?.b} || while` + "if (x) for";\n' + '}';
	assert.deepEqual(units(src), [['f', 1]]);
});

test('arrows, methods, constructors and accessors are units of their own', () => {
	const src = `class K {
	constructor(private x: number) { if (x) {} }
	get y(): number { return this.x > 1 ? 1 : 0; }
	set y(v: number) {}
	m(): number { const g = (v: number) => v > 1 && v < 9; return g(this.x) ? 1 : 0; }
}`;
	assert.deepEqual(units(src), [
		['constructor', 2],
		['y', 2],
		['y', 1],
		['m', 2],
		['(anonymous)', 2]
	]);
});

test('a file with syntax errors reports them', () => {
	const { errors } = measure('probe.ts', 'function broken( {');
	assert.ok(errors.length > 0);
});

function scratch(files) {
	const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ts-ccn-test-'));
	for (const [rel, body] of Object.entries(files)) {
		fs.mkdirSync(path.dirname(path.join(dir, rel)), { recursive: true });
		fs.writeFileSync(path.join(dir, rel), body);
	}
	return dir;
}

test('the report is lizard’s XML shape, and says what it did not measure', () => {
	const dir = scratch({
		'a.ts': 'export function one(x: boolean): number { return x ? 1 : 2; }\n',
		'lib/paraglide/messages.ts': 'export function skipped(): void {}\n',
		'Thing.svelte': '<script lang="ts">let x = 1;</script>\n',
		'types.d.ts': 'declare function ambient(): void;\n'
	});
	const run = spawnSync('node', [tool, dir], { encoding: 'utf-8' });
	assert.equal(run.status, 0, run.stderr);
	const a = path.join(dir, 'a.ts');
	assert.ok(run.stdout.includes(`<item name="one(...) at ${a}:1">\n\t\t\t<value>1</value>\n\t\t\t<value>1</value>\n\t\t\t<value>2</value>`), run.stdout);
	assert.ok(!run.stdout.includes('skipped'));
	assert.ok(!run.stdout.includes('ambient'));
	assert.match(run.stderr, /1 script files, 1 functions measured/);
	assert.match(run.stderr, /not measured — 1 \.svelte components/);
	assert.match(run.stderr, /Thing\.svelte/);
});

test('a file that does not parse fails the run and names the file', () => {
	const dir = scratch({ 'good.ts': 'export function fine(): void {}\n', 'bad.ts': 'function broken( {\n' });
	const run = spawnSync('node', [tool, dir], { encoding: 'utf-8' });
	assert.notEqual(run.status, 0);
	assert.equal(run.stdout, '', 'no partial report');
	assert.match(run.stderr, /bad\.ts: does not parse/);
});

test('the tool runs from any working directory', () => {
	const dir = scratch({ 'a.ts': 'export function one(): void {}\n' });
	const out = execFileSync('node', [tool, 'a.ts'], { cwd: dir, encoding: 'utf-8', stdio: ['ignore', 'pipe', 'ignore'] });
	assert.match(out, /one\(\.\.\.\) at a\.ts:1/);
});

test('a root that does not exist fails the run', () => {
	const run = spawnSync('node', [tool, '/nonexistent/ts-ccn-root'], { encoding: 'utf-8' });
	assert.notEqual(run.status, 0);
	assert.equal(run.stdout, '');
	assert.match(run.stderr, /ts-ccn: \/nonexistent\/ts-ccn-root: /);
});

test('a root with no script files fails the run', () => {
	// A mistyped or emptied root would otherwise score as a language
	// with nothing in it, and the gate would read green on half a
	// repository.
	const dir = scratch({ 'notes.txt': 'nothing here\n' });
	const run = spawnSync('node', [tool, dir], { encoding: 'utf-8' });
	assert.notEqual(run.status, 0);
	assert.equal(run.stdout, '');
	assert.match(run.stderr, /no script files/);
});

test('decisions outside functions are declared in the summary', () => {
	const dir = scratch({ 'a.ts': "if (typeof window !== 'undefined') {}\nexport function one(): void {}\n" });
	const run = spawnSync('node', [tool, dir], { encoding: 'utf-8' });
	assert.equal(run.status, 0, run.stderr);
	assert.match(run.stderr, /1 decisions outside any function not counted/);
});

test('a method name of any text keeps its decisions in its own file', () => {
	// A quoted or computed name can hold the report's own ` at `
	// delimiter, quotes or markup. Emitted raw, the scorer would read
	// the tail of the name as the file and charge the decisions there.
	const dir = scratch({
		'k.ts': [
			'export class K {',
			"\t'parse at runtime'(x: boolean): number { return x ? 1 : 0; }",
			"\t['a\" <b> & c at d:1'](x: boolean): number { if (x) return 1; return 0; }",
			'\t"plain"(): void {}',
			'}',
			''
		].join('\n')
	});
	const report = spawnSync('node', [tool, 'k.ts'], { cwd: dir, encoding: 'utf-8' });
	assert.equal(report.status, 0, report.stderr);
	fs.writeFileSync(path.join(dir, 'ccn.xml'), report.stdout);
	fs.writeFileSync(path.join(dir, 'lcov.info'), ['TN:', 'SF:k.ts', 'LF:1', 'LH:1', 'end_of_record'].join('\n'));
	const scored = JSON.parse(
		execFileSync('node', [path.join(repoRoot, 'tools/crap-score.mjs'), '--ccn=ccn.xml', '--lcov=lcov.info', '--root=.', '--json'], {
			cwd: dir,
			encoding: 'utf-8'
		})
	);
	assert.deepEqual(
		scored.top.map((r) => [r.file, r.ccn]),
		[['k.ts', 5]]
	);
});

// Every file under a root is one of three things: measured, declared
// as not measured in the log, or a failure. A file of a kind nobody
// decided on would otherwise pass through unmeasured and unreported.
test('every script kind the compiler reads is measured', () => {
	const fn = (name) => `export function ${name}(x: boolean) { if (x) { return 1; } return 0; }\n`;
	const js = (name) => `export function ${name}(x) { if (x) { return 1; } return 0; }\n`;
	const dir = scratch({
		'a.ts': fn('a'),
		'b.mts': fn('b'),
		'c.cts': fn('c'),
		'd.tsx': `export function d(x: boolean) { return x ? <i /> : null; }\n`,
		'e.js': js('e'),
		'f.mjs': js('f'),
		'g.cjs': js('g'),
		'h.jsx': `export function h(x) { return x ? <i /> : null; }\n`
	});
	const run = spawnSync('node', [tool, '--tsv', dir], { encoding: 'utf-8' });
	assert.equal(run.status, 0, run.stderr);
	const got = run.stdout
		.trim()
		.split('\n')
		.map((l) => l.split('\t'))
		.map(([file, , name, , , ccn]) => [path.basename(file), name, Number(ccn)]);
	assert.deepEqual(got, [
		['a.ts', 'a', 2],
		['b.mts', 'b', 2],
		['c.cts', 'c', 2],
		['d.tsx', 'd', 2],
		['e.js', 'e', 2],
		['f.mjs', 'f', 2],
		['g.cjs', 'g', 2],
		['h.jsx', 'h', 2]
	]);
});

test('files that hold no script are listed as not measured', () => {
	const dir = scratch({ 'a.ts': 'export function a(): void {}\n', 'app.css': 'a {}\n', 'app.html': '<p></p>\n', 'x.json': '{}\n' });
	const run = spawnSync('node', [tool, dir], { encoding: 'utf-8' });
	assert.equal(run.status, 0, run.stderr);
	for (const f of ['app.css', 'app.html', 'x.json']) assert.ok(run.stderr.includes(path.join(dir, f)), run.stderr);
	assert.match(run.stderr, /not measured — 3 files that hold no script/);
});

test('generated Paraglide output is declared as skipped', () => {
	const dir = scratch({ 'a.ts': 'export function a(): void {}\n', 'lib/paraglide/messages.js': 'export function m() {}\n' });
	const run = spawnSync('node', [tool, dir], { encoding: 'utf-8' });
	assert.equal(run.status, 0, run.stderr);
	assert.match(run.stderr, /skipped — 1 files of generated Paraglide output/);
});

test('a file of a kind nobody decided on fails the run and names it', () => {
	const dir = scratch({ 'a.ts': 'export function a(): void {}\n', 'b.coffee': 'f = -> 1\n' });
	const run = spawnSync('node', [tool, dir], { encoding: 'utf-8' });
	assert.notEqual(run.status, 0);
	assert.equal(run.stdout, '');
	assert.match(run.stderr, /b\.coffee: neither measured nor declared/);
});

test('markup carrying an inline script is not taken for markup without one', () => {
	const dir = scratch({ 'a.ts': 'export function a(): void {}\n', 'app.html': '<body><script>if (x) {}</script></body>\n' });
	const run = spawnSync('node', [tool, dir], { encoding: 'utf-8' });
	assert.notEqual(run.status, 0);
	assert.match(run.stderr, /app\.html: neither measured nor declared/);
});
