// CRAP (Change Risk Anti-Pattern) score per file.
//
// Formula: CRAP(f) = ccn(f)² × (1 − cov(f))² + ccn(f)
// where ccn = sum of cyclomatic complexity per function in the file
// and cov = line coverage % for that file (0..1).
//
// We aggregate per FILE rather than per function — function-level
// CRAP needs precise line ranges that lizard's XML doesn't expose,
// and the per-file signal already surfaces the same anti-pattern:
// a file with high total complexity AND poor coverage is the place
// to look.
//
// Inputs:
//   - lizard XML on stdin (run: `lizard --xml <paths> | crap-score.mjs`)
//   - lcov.info path via `--lcov=<path>[:<prefix>]` (one or more times).
//     The optional `:prefix` is prepended to every relative SF: path
//     in that lcov so paths line up across the repo. Frontend lcov
//     uses relative paths (under frontend/); Rust lcov uses
//     absolute paths (handled separately).
//   - --root=<repoRoot> to make file paths comparable across inputs
//
// Filters out test files (*.test.{ts,js}, *_test.rs, tests/) — lizard
// counts their complexity but lcov never covers them, so they'd
// dominate the CRAP rankings as artifacts.
//
// Output: a sorted-by-CRAP-desc table on stdout, plus aggregate
// metrics (max, p95, count > 30) on stderr's last line. `--json`
// emits just the aggregates as JSON (used by the ratchet check).

import fs from 'node:fs';
import path from 'node:path';

const args = process.argv.slice(2);
const lcovPaths = args.filter((a) => a.startsWith('--lcov=')).map((a) => a.slice('--lcov='.length));
const rootFlag = args.find((a) => a.startsWith('--root='));
const root = rootFlag ? rootFlag.slice('--root='.length) : process.cwd();
const jsonFlag = args.includes('--json');

if (lcovPaths.length === 0) {
	console.error('usage: lizard --xml <paths> | crap-score.mjs --lcov=<path> [--lcov=<path> ...] [--root=<dir>] [--json]');
	process.exit(2);
}

/** Parse lizard's XML output for per-file complexity totals. */
function parseLizardXml(xml) {
	// Each function is <item name="fn(...) at file:line"><value>nr</value><value>NCSS</value><value>CCN</value></item>
	const re = /<item name="[^"]*?at ([^:]+):\d+">\s*<value>\d+<\/value>\s*<value>\d+<\/value>\s*<value>(\d+)<\/value>/g;
	/** @type {Map<string, number>} */
	const ccnByFile = new Map();
	let m;
	while ((m = re.exec(xml)) !== null) {
		const file = path.normalize(m[1]);
		const ccn = Number(m[2]);
		ccnByFile.set(file, (ccnByFile.get(file) ?? 0) + ccn);
	}
	return ccnByFile;
}

/** Parse one lcov.info, return { file → { LF, LH } } keyed by repo-relative path. */
function parseLcov(file, prefix = '') {
	const text = fs.readFileSync(file, 'utf-8');
	/** @type {Map<string, { LF: number, LH: number }>} */
	const byFile = new Map();
	let cur = null;
	for (const raw of text.split('\n')) {
		const line = raw.trim();
		if (line.startsWith('SF:')) {
			let p = line.slice(3);
			if (path.isAbsolute(p)) {
				p = path.relative(root, p);
			} else if (prefix) {
				p = path.join(prefix, p);
			}
			cur = { file: path.normalize(p), LF: 0, LH: 0, daLines: 0, daHit: 0 };
		} else if (line.startsWith('DA:') && cur) {
			// Per-line data is the ground truth: toolchains have been
			// caught emitting LF/LH summaries that disagree with their
			// own DA lines (LH under-reporting a fully covered file),
			// and a summary-trusting score invents risk that isn't
			// there.
			cur.daLines += 1;
			if (Number(line.slice(3).split(',')[1]) > 0) cur.daHit += 1;
		} else if (line.startsWith('LF:') && cur) cur.LF = Number(line.slice(3));
		else if (line.startsWith('LH:') && cur) cur.LH = Number(line.slice(3));
		else if (line === 'end_of_record' && cur) {
			const entry = cur.daLines > 0 ? { LF: cur.daLines, LH: cur.daHit } : { LF: cur.LF, LH: cur.LH };
			byFile.set(cur.file, entry);
			cur = null;
		}
	}
	return byFile;
}

/** Should this file count toward CRAP? Excludes test files since lizard
 *  scores their complexity but lcov never covers them — they'd
 *  artifact-dominate the rankings. */
function isProductionFile(file) {
	if (/\.(test|spec)\.[jt]sx?$/.test(file)) return false;
	if (/(^|\/)tests?\//.test(file)) return false;
	if (/_test\.rs$/.test(file)) return false;
	return true;
}

const xml = fs.readFileSync(0, 'utf-8');
const ccnByFile = parseLizardXml(xml);
/** @type {Map<string, { LF: number, LH: number }>} */
const cov = new Map();
for (const spec of lcovPaths) {
	const [p, prefix = ''] = spec.split(':');
	for (const [file, entry] of parseLcov(p, prefix)) cov.set(file, entry);
}

const rows = [];
for (const [file, ccn] of ccnByFile) {
	if (!isProductionFile(file)) continue;
	const c = cov.get(file);
	// Files lizard saw but no lcov entry → assume zero coverage, full
	// risk. Conversely lcov-only files have no complexity to reason
	// about; skip those.
	const lf = c?.LF ?? 0;
	const lh = c?.LH ?? 0;
	const coverage = lf > 0 ? lh / lf : 0;
	const crap = ccn * ccn * Math.pow(1 - coverage, 2) + ccn;
	rows.push({ file, ccn, lf, lh, coverage, crap });
}
rows.sort((a, b) => b.crap - a.crap);

/** Fewest rows of the ranking `--json` carries for diagnostics. */
const TOP_N = 10;

const max = rows[0]?.crap ?? 0;
const sorted = rows.map((r) => r.crap).sort((a, b) => a - b);
const p95 = sorted[Math.floor(0.95 * (sorted.length - 1))] ?? 0;
// Same percentile counted from the worst end, which is the order the
// report is in. It slides further down as the file count grows — past
// 181 files it is already below a ten-row report — so the report has
// to be sized to reach it rather than to a fixed length.
const p95Index = rows.length ? rows.length - 1 - Math.floor(0.95 * (rows.length - 1)) : 0;
const high_risk = rows.filter((r) => r.crap > 30).length;

if (jsonFlag) {
	// `high_risk_files` names what the count is counting. Without it a
	// regression reports "26 vs 25" and the operator cannot tell which
	// file crossed — and the answer is not always reproducible
	// locally, since a file sitting a fraction under 30 crosses on a
	// coverage difference between environments.
	//
	// `top` is the same argument for the other two metrics. `max` is
	// by definition `top[0].crap` and `p95` is a rank in this same
	// ordering, so a regression in either is only diagnosable against
	// the ranking. It cannot come from `high_risk_files`: max can move
	// while nothing is over the bar at all, and that list is then
	// empty.
	//
	// Length is `TOP_N` or as far as the percentile, whichever reaches
	// further. A fixed cap would drop the p95 row on any repo past 181
	// files and leave a p95 failure printing ten rows that are not the
	// one to fix. `p95_file` names it so the reader can tell the
	// percentile from the merely-worse.
	const report = (r) => ({ file: r.file, ccn: r.ccn, cov: r2(r.coverage * 100), crap: r2(r.crap) });
	process.stdout.write(
		JSON.stringify({
			max: r2(max),
			p95: r2(p95),
			high_risk,
			count: rows.length,
			high_risk_files: rows.filter((r) => r.crap > 30).map(report),
			top: rows.slice(0, Math.max(TOP_N, p95Index + 1)).map(report),
			p95_file: rows[p95Index]?.file ?? null
		}) + '\n'
	);
} else {
	const widths = { file: 50, ccn: 6, cov: 8, crap: 8 };
	console.log(`${pad('file', widths.file)}${pad('ccn', widths.ccn)}${pad('cov', widths.cov)}${pad('crap', widths.crap)}`);
	console.log('-'.repeat(72));
	for (const r of rows.slice(0, 20)) {
		console.log(
			pad(r.file, widths.file) +
				pad(String(r.ccn), widths.ccn) +
				pad((100 * r.coverage).toFixed(1) + '%', widths.cov) +
				pad(r.crap.toFixed(1), widths.crap)
		);
	}
	console.error(`\nmax=${r2(max)}  p95=${r2(p95)}  high_risk(>30)=${high_risk}  files=${rows.length}`);
}

function r2(x) {
	return Math.round(x * 100) / 100;
}
function pad(s, w) {
	const str = String(s);
	if (str.length > w - 1) return str.slice(0, w - 4) + '... ';
	return str + ' '.repeat(Math.max(0, w - str.length));
}
