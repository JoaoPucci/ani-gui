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
import { execFileSync, spawnSync } from 'node:child_process';
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

/** A record carrying real per-line DA data alongside its summary. */
function lcovRecordWithDa(file, daHits, LF, LH) {
	const da = daHits.map((c, i) => `DA:${i + 1},${c}`);
	return ['TN:', `SF:${file}`, ...da, `LF:${LF}`, `LH:${LH}`, 'end_of_record'].join('\n');
}

test('per-line DA data outranks a disagreeing LF/LH summary', () => {
	// Captured live from a ratchet run: the runner's toolchain emitted
	// LH=96 for a record whose 139 DA lines all show hits. The summary
	// under-reports, the penalty term invents uncovered lines that do
	// not exist, and a fully covered file reads as high-risk. Per-line
	// data is the ground truth; the summary is only a fallback for
	// records that carry no DA lines at all.
	const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'crap-score-da-'));
	const xml = ['<root><cppncss><measure type="Function">', lizardItem('src/d.rs', 6), '</measure></cppncss></root>'].join('\n');
	fs.writeFileSync(path.join(tmpDir, 'lizard.xml'), xml);
	fs.writeFileSync(
		path.join(tmpDir, 'lcov.info'),
		lcovRecordWithDa('src/d.rs', [1, 1, 1, 1, 1, 1, 1, 1, 1, 1], 10, 5)
	);
	const out = JSON.parse(
		execFileSync('node', [scriptUnderTest, '--lcov=lcov.info', '--root=.', '--json'], {
			cwd: tmpDir,
			encoding: 'utf-8',
			input: fs.readFileSync(path.join(tmpDir, 'lizard.xml'), 'utf-8')
		})
	);
	assert.equal(out.top[0].cov, 100, 'ten DA lines, ten hits: the file is fully covered');
	assert.equal(out.top[0].crap, 6, 'full coverage leaves only the bare complexity');
});

// The gate measures each language with its own parser, so complexity
// arrives as one report per tool. Named on the command line, every
// report is read; a report that is not there is a measurement that did
// not happen, and the score refuses rather than ranking without it.
test('--ccn reads every complexity report it is given', () => {
	const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'crap-score-ccn-'));
	const report = (items) => ['<?xml version="1.0" ?>', '<cppncss><measure type="Function">', ...items, '</measure></cppncss>'].join('\n');
	fs.writeFileSync(path.join(tmpDir, 'rust.xml'), report([lizardItem('backend/src/a.rs', 4)]));
	fs.writeFileSync(path.join(tmpDir, 'ts.xml'), report([lizardItem('frontend/src/b.ts', 3), lizardItem('frontend/src/b.ts', 2)]));
	fs.writeFileSync(path.join(tmpDir, 'lcov.info'), [lcovRecord('backend/src/a.rs', 10, 10), lcovRecord('frontend/src/b.ts', 10, 10)].join('\n'));
	const out = JSON.parse(
		execFileSync('node', [scriptUnderTest, '--ccn=rust.xml', '--ccn=ts.xml', '--lcov=lcov.info', '--root=.', '--json'], {
			cwd: tmpDir,
			encoding: 'utf-8',
			stdio: ['ignore', 'pipe', 'pipe']
		})
	);
	assert.equal(out.count, 2);
	assert.deepEqual(
		out.top.map((r) => [r.file, r.ccn]),
		[
			['frontend/src/b.ts', 5],
			['backend/src/a.rs', 4]
		]
	);
});

test('--ccn naming a missing report fails instead of scoring without it', () => {
	const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'crap-score-ccn-missing-'));
	fs.writeFileSync(path.join(tmpDir, 'lcov.info'), lcovRecord('src/a.rs', 10, 10));
	const run = spawnSync('node', [scriptUnderTest, '--ccn=absent.xml', '--lcov=lcov.info', '--root=.', '--json'], {
		cwd: tmpDir,
		encoding: 'utf-8'
	});
	assert.notEqual(run.status, 0);
	assert.match(run.stderr, /absent\.xml/);
});

// An item's name attribute is `<function>(...) at <file>:<line>`, XML-
// escaped. The producers' function names carry no whitespace, so the
// first ` at ` ends the name and everything up to the last `:` is the
// file — whatever the file's path contains.
test('a file path holding the delimiter, colons or escaped characters is read whole', () => {
	const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'crap-score-path-'));
	const file = 'src/a at b/x:y & "z" <w>.ts';
	const escaped = file.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
	const xml = [
		'<cppncss><measure type="Function">',
		`<item name="f(...) at ${escaped}:3"><value>1</value><value>1</value><value>4</value></item>`,
		`<item name="g(...) at ${escaped}:9"><value>2</value><value>1</value><value>2</value></item>`,
		'</measure></cppncss>'
	].join('\n');
	fs.writeFileSync(path.join(tmpDir, 'ccn.xml'), xml);
	fs.writeFileSync(path.join(tmpDir, 'lcov.info'), lcovRecord(file, 10, 10));
	const out = JSON.parse(
		execFileSync('node', [scriptUnderTest, '--ccn=ccn.xml', '--lcov=lcov.info', '--root=.', '--json'], {
			cwd: tmpDir,
			encoding: 'utf-8'
		})
	);
	assert.deepEqual(
		out.top.map((r) => [r.file, r.ccn, r.cov]),
		[[path.normalize(file), 6, 100]]
	);
});

test('an item that names no file fails the run instead of dropping its complexity', () => {
	const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'crap-score-unplaced-'));
	const xml = '<cppncss><measure type="Function"><item name="f(...)"><value>1</value><value>1</value><value>4</value></item></measure></cppncss>';
	fs.writeFileSync(path.join(tmpDir, 'ccn.xml'), xml);
	fs.writeFileSync(path.join(tmpDir, 'lcov.info'), lcovRecord('src/a.rs', 10, 10));
	const run = spawnSync('node', [scriptUnderTest, '--ccn=ccn.xml', '--lcov=lcov.info', '--root=.', '--json'], {
		cwd: tmpDir,
		encoding: 'utf-8'
	});
	assert.notEqual(run.status, 0);
	assert.match(run.stderr, /f\(\.\.\.\)/);
});

// Every <item> in a report is complexity; one the scorer cannot read
// whole — another attribute layout, a line that is not a number, a
// value that is not a count — would otherwise vanish from the totals.
for (const [why, item] of [
	['an unexpected layout', '<item name="k(...) at e.ts:1" ><value>1</value><value>1</value><value>5</value></item>'],
	['a negative count', '<item name="k(...) at e.ts:1"><value>1</value><value>1</value><value>-1</value></item>'],
	['no line number', '<item name="f(...) at a:b.ts"><value>1</value><value>1</value><value>5</value></item>'],
	['a line that is not a number', '<item name="g(...) at c.ts:x"><value>1</value><value>1</value><value>5</value></item>']
]) {
	test(`an item with ${why} fails the run instead of dropping its complexity`, () => {
		const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'crap-score-shape-'));
		fs.writeFileSync(path.join(tmpDir, 'ccn.xml'), `<cppncss><measure type="Function">${item}</measure></cppncss>`);
		fs.writeFileSync(path.join(tmpDir, 'lcov.info'), lcovRecord('e.ts', 10, 10));
		const run = spawnSync('node', [scriptUnderTest, '--ccn=ccn.xml', '--lcov=lcov.info', '--root=.', '--json'], {
			cwd: tmpDir,
			encoding: 'utf-8'
		});
		assert.notEqual(run.status, 0, run.stdout);
	});
}

test('numeric character references in a path are decoded like named ones', () => {
	const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'crap-score-numeric-'));
	const xml = `<cppncss><measure type="Function"><item name="f(...) at a&#39;b&#x26;c.ts:1"><value>1</value><value>1</value><value>3</value></item></measure></cppncss>`;
	fs.writeFileSync(path.join(tmpDir, 'ccn.xml'), xml);
	fs.writeFileSync(path.join(tmpDir, 'lcov.info'), lcovRecord("a'b&c.ts", 10, 10));
	const out = JSON.parse(
		execFileSync('node', [scriptUnderTest, '--ccn=ccn.xml', '--lcov=lcov.info', '--root=.', '--json'], { cwd: tmpDir, encoding: 'utf-8' })
	);
	assert.deepEqual(out.top.map((r) => [r.file, r.ccn, r.cov]), [["a'b&c.ts", 3, 100]]);
});

test('test files of every measured script kind are left out of the ranking', () => {
	const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'crap-score-tests-'));
	const files = ['src/a.test.mjs', 'src/b.spec.cjs', 'src/c.test.mts', 'src/d.test.cts', 'src/e.test.tsx', 'src/keep.mjs'];
	const xml = ['<cppncss><measure type="Function">', ...files.map((f) => lizardItem(f, 5)), '</measure></cppncss>'].join('\n');
	fs.writeFileSync(path.join(tmpDir, 'ccn.xml'), xml);
	fs.writeFileSync(path.join(tmpDir, 'lcov.info'), lcovRecord('src/keep.mjs', 10, 10));
	const out = JSON.parse(
		execFileSync('node', [scriptUnderTest, '--ccn=ccn.xml', '--lcov=lcov.info', '--root=.', '--json'], { cwd: tmpDir, encoding: 'utf-8' })
	);
	assert.deepEqual(out.top.map((r) => r.file), ['src/keep.mjs']);
});

// A report is read whole or not at all. A counter that exits 0 after
// writing a truncated report would otherwise have the functions it
// managed to write scored and the rest silently absent, and the firm
// ceiling could pass on a file that was never measured.
for (const [why, xml] of [
	[
		'an item that never closes',
		'<cppncss><measure type="Function"><item name="f(...) at e.ts:1"><value>1</value><value>1</value><value>4</value><item name="g(...) at e.ts:9"><value>2</value><value>1</value><value>4</value></item></measure></cppncss>'
	],
	[
		'a report cut off after a count',
		'<?xml version="1.0" ?>\n<cppncss>\n\t<measure type="Function">\n\t\t<item name="f(...) at e.ts:1">\n\t\t\t<value>1</value>\n\t\t\t<value>1</value>\n\t\t\t<value>4</value>'
	],
	[
		'a report cut off after its last item',
		'<cppncss><measure type="Function"><item name="f(...) at e.ts:1"><value>1</value><value>1</value><value>4</value></item>'
	]
]) {
	test(`${why} fails the run instead of scoring what was read`, () => {
		const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'crap-score-whole-'));
		fs.writeFileSync(path.join(tmpDir, 'ccn.xml'), xml);
		fs.writeFileSync(path.join(tmpDir, 'lcov.info'), lcovRecord('e.ts', 10, 10));
		const run = spawnSync('node', [scriptUnderTest, '--ccn=ccn.xml', '--lcov=lcov.info', '--root=.', '--json'], {
			cwd: tmpDir,
			encoding: 'utf-8'
		});
		assert.notEqual(run.status, 0, run.stdout);
	});
}

// The same for coverage: a record that never reaches its
// `end_of_record` — the file cut off, or the next record opening over
// it — would otherwise be dropped without a word, and its file scored
// at no coverage or under the wrong one.
for (const [why, lcov] of [
	['a coverage file cut off inside a record', ['TN:', 'SF:e.ts', 'DA:1,1', 'LF:1', 'LH:1'].join('\n')],
	['a coverage record opening before the previous one closed', ['TN:', 'SF:a.ts', 'DA:1,1', 'SF:e.ts', 'DA:1,1', 'end_of_record'].join('\n')]
]) {
	test(`${why} fails the run instead of scoring without it`, () => {
		const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'crap-score-lcov-whole-'));
		fs.writeFileSync(path.join(tmpDir, 'ccn.xml'), `<cppncss><measure type="Function">${lizardItem('e.ts', 4)}</measure></cppncss>`);
		fs.writeFileSync(path.join(tmpDir, 'lcov.info'), lcov);
		const run = spawnSync('node', [scriptUnderTest, '--ccn=ccn.xml', '--lcov=lcov.info', '--root=.', '--json'], {
			cwd: tmpDir,
			encoding: 'utf-8'
		});
		assert.notEqual(run.status, 0, run.stdout);
		assert.match(run.stderr, /end_of_record/);
	});
}

// The high-risk count sits at its ceiling, so the files just under
// the bar decide the gate on a sliver of coverage. The table is where
// they are read from — in the build log and locally — so it has to
// reach them, however many files sit above the bar.
test('the table reaches every file within five of the high-risk bar', () => {
	const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'crap-score-table-'));
	const items = [];
	const records = [];
	// Twenty-four files over the bar fill the old twenty-row table on
	// their own; the one just under the bar comes after all of them.
	for (let i = 0; i < 24; i++) {
		const file = `src/over${String(i).padStart(2, '0')}.rs`;
		items.push(lizardItem(file, 40));
		records.push(lcovRecord(file, 10, 10));
	}
	items.push(lizardItem('src/near.rs', 29), lizardItem('src/far.rs', 20));
	records.push(lcovRecord('src/near.rs', 10, 10), lcovRecord('src/far.rs', 10, 10));
	fs.writeFileSync(path.join(tmpDir, 'ccn.xml'), `<cppncss><measure type="Function">${items.join('')}</measure></cppncss>`);
	fs.writeFileSync(path.join(tmpDir, 'lcov.info'), records.join('\n'));
	const table = execFileSync('node', [scriptUnderTest, '--ccn=ccn.xml', '--lcov=lcov.info', '--root=.'], {
		cwd: tmpDir,
		encoding: 'utf-8',
		stdio: ['ignore', 'pipe', 'ignore']
	});
	assert.match(table, /src\/near\.rs/, 'a file within five of the bar must be in the table');
	assert.doesNotMatch(table, /src\/far\.rs/, 'a file well under the bar is not');
});
