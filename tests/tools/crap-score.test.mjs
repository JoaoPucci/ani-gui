// Pin the JSON contract of tools/crap-score.mjs.
//
// The ratchet consumes this JSON, so anything the ratchet has to
// report on has to be in it. `high_risk_files` covers the high-risk
// COUNT, but `max` and `p95` are their own metrics with their own
// baselines and they can regress while nothing at all is over the
// high-risk bar — at which point the summary carries no file names
// and the failure is a bare number.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../..');
const scriptUnderTest = path.join(repoRoot, 'tools/crap-score.mjs');

/** One lizard `<item>` in the shape the XML parser matches. */
function lizardItem(file, ccn) {
	return `<item name="f() at ${file}:1"><value>1</value><value>1</value><value>${ccn}</value></item>`;
}

function lcovRecord(file, LF, LH) {
	return ['TN:', `SF:${file}`, `LF:${LF}`, `LH:${LH}`, 'end_of_record'].join('\n');
}

/**
 * Three files, none of them over the high-risk bar:
 *
 *   b.rs  ccn 4, cov 50%  -> 4² × 0.5² + 4  = 8    <- max
 *   a.rs  ccn 6, cov 80%  -> 6² × 0.2² + 6  = 7.44
 *   c.rs  ccn 3, cov 100% -> 3             = 3
 *
 * so `high_risk` is 0 and `high_risk_files` is empty, while `max`
 * still has a definite owner.
 */
function stageFixture() {
	const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'crap-score-test-'));
	const xml = ['<root><cppncss><measure type="Function">', lizardItem('src/a.rs', 6), lizardItem('src/b.rs', 4), lizardItem('src/c.rs', 3), '</measure></cppncss></root>'].join('\n');
	fs.writeFileSync(path.join(tmpDir, 'lizard.xml'), xml);
	fs.writeFileSync(
		path.join(tmpDir, 'lcov.info'),
		[lcovRecord('src/a.rs', 10, 8), lcovRecord('src/b.rs', 10, 5), lcovRecord('src/c.rs', 10, 10)].join('\n')
	);

	const runJson = () =>
		JSON.parse(
			execFileSync('node', [scriptUnderTest, '--lcov=lcov.info', '--root=.', '--json'], {
				cwd: tmpDir,
				encoding: 'utf-8',
				input: fs.readFileSync(path.join(tmpDir, 'lizard.xml'), 'utf-8')
			})
		);

	return { tmpDir, runJson };
}

test('--json reports the aggregates it always did', () => {
	const { runJson } = stageFixture();
	const out = runJson();
	assert.equal(out.max, 8);
	assert.equal(out.high_risk, 0);
	assert.equal(out.count, 3);
	assert.deepEqual(out.high_risk_files, []);
});

test('--json names the top of the ranking even when nothing is high-risk', () => {
	const { runJson } = stageFixture();
	const out = runJson();
	assert.ok(Array.isArray(out.top), 'the summary must carry a `top` ranking');
	assert.ok(out.top.length > 0, '`top` must not be empty when files were scored');
	assert.equal(
		out.top[0].file,
		'src/b.rs',
		'`top[0]` must be the file that set `max`, which here is under the high-risk bar'
	);
	assert.equal(out.top[0].crap, out.max, '`top[0].crap` and `max` are the same number');
	assert.equal(out.top[0].ccn, 4);
	assert.equal(out.top[0].cov, 50);
});

test('--json orders `top` by CRAP, worst first', () => {
	const { runJson } = stageFixture();
	const craps = runJson().top.map((r) => r.crap);
	assert.deepEqual(craps, [...craps].sort((x, y) => y - x), '`top` must be sorted descending');
});

/**
 * `n` files, each with zero coverage and a distinct complexity, so the
 * descending ranking is exactly the order they are generated in.
 */
function stageWideFixture(n) {
	const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'crap-score-wide-'));
	const items = [];
	const records = [];
	for (let i = 0; i < n; i++) {
		const file = `src/f${String(i).padStart(4, '0')}.rs`;
		items.push(lizardItem(file, n - i));
		records.push(lcovRecord(file, 10, 0));
	}
	fs.writeFileSync(
		path.join(tmpDir, 'lizard.xml'),
		['<root><cppncss><measure type="Function">', ...items, '</measure></cppncss></root>'].join('\n')
	);
	fs.writeFileSync(path.join(tmpDir, 'lcov.info'), records.join('\n'));

	return JSON.parse(
		execFileSync('node', [scriptUnderTest, '--lcov=lcov.info', '--root=.', '--json'], {
			cwd: tmpDir,
			encoding: 'utf-8',
			input: fs.readFileSync(path.join(tmpDir, 'lizard.xml'), 'utf-8')
		})
	);
}

// p95 is picked from the ASCENDING ordering, so its position counted
// from the worst end is `n - 1 - floor(0.95 * (n - 1))` — which slides
// further down as the file count grows. At 182 scored files it is the
// 11th row and falls off a ten-row report entirely, so a p95-only
// failure would print ten worse files and still never name the one at
// the boundary. p95 is a firm ceiling that has to be fixed rather than
// re-baselined, so the row that sets it has to be in the report.
//
// The repo scored 175 files when this was written. Seven files from
// the diagnostic going quiet on exactly the metric it was added for.
test('--json carries the p95 row even when it sits below the worst ten', () => {
	const out = stageWideFixture(200);
	const expectedIndex = out.count - 1 - Math.floor(0.95 * (out.count - 1));
	assert.ok(expectedIndex > 9, 'fixture must put p95 outside a ten-row report');

	assert.ok(
		out.top.some((r) => r.crap === out.p95),
		`no row in \`top\` carries the reported p95 (${out.p95}); worst was ${out.top[0]?.crap}, ` +
			`report holds ${out.top.length} rows and p95 is at index ${expectedIndex}`
	);
});

test('--json names the file at the p95 boundary', () => {
	const out = stageWideFixture(200);
	const expectedIndex = out.count - 1 - Math.floor(0.95 * (out.count - 1));
	assert.equal(
		out.p95_file,
		`src/f${String(expectedIndex).padStart(4, '0')}.rs`,
		'`p95_file` must name the row at the percentile boundary'
	);
	assert.equal(
		out.top[expectedIndex]?.file,
		out.p95_file,
		'and that row must be reachable in `top` at its own rank'
	);
});
