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
