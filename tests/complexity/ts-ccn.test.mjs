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
	assert.match(run.stderr, /1 TypeScript files, 1 functions measured/);
	assert.match(run.stderr, /not measured — 1 \.svelte components/);
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
