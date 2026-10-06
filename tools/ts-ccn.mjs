#!/usr/bin/env node
// Per-function cyclomatic complexity for TypeScript, on the compiler's
// own parse of each file.
//
// The CRAP gate used to take its frontend complexity from lizard,
// whose TypeScript support is a regex tokenizer driving a small state
// machine. It loses its place on ordinary syntax — a function with a
// return-type annotation followed directly by another function is
// enough — and when it does it folds the functions that follow into
// the one it was reading, without saying so. The gate under-measured
// while reading green.
//
// Here the TypeScript compiler parses each file. Every function-like
// node with a body is a unit: declarations, methods, constructors,
// accessors, function expressions and arrow functions. The count
// within a unit is lizard's documented decision set, kept so the
// numbers stay continuous with the ceilings recorded against it: 1,
// plus one for each `if`, `for`, `while`, `catch` and `case` keyword,
// each conditional `? :`, and each `&&` and `||` (and their `&&=` and
// `||=` forms). The tokens are the
// parser's, so a keyword inside a string or a template is text, a
// promise's `.catch(` is a method name, and neither `?.`, `??` nor the
// `?` of an optional parameter is a conditional — lizard's tokenizer
// counted some of those. A function nested in another is its own unit,
// and its tokens are not counted again in the enclosing one.
//
// Every file under a root lands in one place. Script of any kind the
// compiler reads (.ts .mts .cts .tsx .js .mjs .cjs .jsx) is measured.
// What is not measured is listed on stderr: `.svelte` components (their
// script blocks are not script files, and the coverage run does not
// instrument them either), declaration files (no bodies), and markup,
// styles and data (.html without an inline <script>, .css, .json);
// generated Paraglide output is skipped and counted. A file of any
// other kind fails the run. Decisions outside every function
// (module-scope code) are counted and reported as charged to no unit.
//
// Usage: node tools/ts-ccn.mjs [--tsv] <path>...
// Default output is lizard's XML shape, which tools/crap-score.mjs
// reads; `--tsv` lists one unit per line. A file with syntax errors is
// reported and the run exits non-zero — an unparsed file is a
// measurement that did not happen, and the gate must not read it as
// zero. So is a root that does not exist or holds no TypeScript.
// Paths containing `paraglide` (compiled message bundles) are
// skipped, as the gate always skipped them.

import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
// The compiler the frontend already builds with, not a second copy.
const ts = createRequire(path.join(repoRoot, 'frontend/package.json'))('typescript');

const FUNCTION_KINDS = new Set([
	ts.SyntaxKind.FunctionDeclaration,
	ts.SyntaxKind.MethodDeclaration,
	ts.SyntaxKind.Constructor,
	ts.SyntaxKind.GetAccessor,
	ts.SyntaxKind.SetAccessor,
	ts.SyntaxKind.FunctionExpression,
	ts.SyntaxKind.ArrowFunction
]);

const DECISION_KINDS = new Set([
	ts.SyntaxKind.IfKeyword,
	ts.SyntaxKind.ForKeyword,
	ts.SyntaxKind.WhileKeyword,
	ts.SyntaxKind.CatchKeyword,
	ts.SyntaxKind.CaseKeyword,
	ts.SyntaxKind.AmpersandAmpersandToken,
	ts.SyntaxKind.BarBarToken,
	// `&&=` and `||=` short-circuit as `&&` and `||` do; lizard's
	// tokenizer counted them as those operators.
	ts.SyntaxKind.AmpersandAmpersandEqualsToken,
	ts.SyntaxKind.BarBarEqualsToken
]);

/** A leaf token that adds a path. A `?` does only as a conditional's:
 *  the one marking an optional parameter or property is a type. */
function isDecision(token) {
	if (token.kind === ts.SyntaxKind.QuestionToken) return token.parent.kind === ts.SyntaxKind.ConditionalExpression;
	return DECISION_KINDS.has(token.kind);
}

/**
 * Measure one source text.
 * `outside` counts the decisions that sit in no function — a
 * module-scope guard — which no unit is charged with, as lizard
 * charged them to none, and which the report declares.
 * @returns {{ units: { name: string, line: number, endLine: number, ccn: number }[], outside: number, errors: string[] }}
 */
export function measure(fileName, source) {
	const sf = ts.createSourceFile(fileName, source, ts.ScriptTarget.Latest, true, scriptKind(fileName));
	const errors = sf.parseDiagnostics.map((d) => {
		const at = sf.getLineAndCharacterOfPosition(d.start ?? 0);
		return `${at.line + 1}:${at.character + 1}: ${ts.flattenDiagnosticMessageText(d.messageText, '\n')}`;
	});
	const units = [];
	const decisions = [];
	const walk = (node) => {
		if (node.kind >= ts.SyntaxKind.FirstJSDocNode && node.kind <= ts.SyntaxKind.LastJSDocNode) return;
		if (FUNCTION_KINDS.has(node.kind) && node.body) {
			units.push({ name: node.kind === ts.SyntaxKind.Constructor ? 'constructor' : node.name ? node.name.getText(sf) : '(anonymous)', start: node.getStart(sf), end: node.end, own: 0 });
		}
		const children = node.getChildren(sf);
		if (children.length === 0 && isDecision(node)) decisions.push(node.getStart(sf));
		for (const child of children) walk(child);
	};
	walk(sf);
	let outside = 0;
	for (const at of decisions) {
		let innermost = null;
		for (const u of units) {
			if (u.start <= at && at < u.end && (innermost === null || u.start > innermost.start)) innermost = u;
		}
		if (innermost) innermost.own += 1;
		else outside += 1;
	}
	return {
		errors,
		outside,
		units: units
			.map((u) => ({
				name: u.name,
				line: sf.getLineAndCharacterOfPosition(u.start).line + 1,
				endLine: sf.getLineAndCharacterOfPosition(u.end).line + 1,
				ccn: 1 + u.own
			}))
			.sort((a, b) => a.line - b.line || a.endLine - b.endLine)
	};
}

/** The parse a file gets, by extension. JSX needs its own kinds. */
function scriptKind(fileName) {
	if (/\.tsx$/.test(fileName)) return ts.ScriptKind.TSX;
	if (/\.jsx$/.test(fileName)) return ts.ScriptKind.JSX;
	if (/\.[mc]?js$/.test(fileName)) return ts.ScriptKind.JS;
	return ts.ScriptKind.TS;
}

// Every file under a root lands in exactly one place. A kind not
// named here fails the run, so a new kind gets a decision rather than
// passing through unmeasured and unreported.
const KINDS = [
	// Declarations carry no function bodies to measure.
	['declaration', /\.d\.[mc]?ts$/],
	['script', /\.([mc]?ts|tsx|[mc]?js|jsx)$/],
	['svelte', /\.svelte$/],
	// Markup, styles and data: no script of either language.
	['noScript', /\.(css|html|json)$/]
];

function collect(p, out) {
	// Paraglide's compiled message bundles: generated, one switch arm per
	// message key, and never measured by the gate.
	if (p.includes('paraglide')) {
		out.paraglide += fs.statSync(p).isDirectory() ? countFiles(p) : 1;
		return;
	}
	const stat = fs.statSync(p);
	if (stat.isDirectory()) {
		for (const entry of fs.readdirSync(p).sort()) collect(path.join(p, entry), out);
		return;
	}
	let kind = KINDS.find(([, re]) => re.test(p))?.[0] ?? 'unknown';
	// Markup is only script-free when it carries no inline <script>.
	if (kind === 'noScript' && p.endsWith('.html') && /<script\b/i.test(fs.readFileSync(p, 'utf-8'))) kind = 'unknown';
	out[kind].push(p);
}

function countFiles(dir) {
	return fs.readdirSync(dir, { withFileTypes: true }).reduce((n, e) => n + (e.isDirectory() ? countFiles(path.join(dir, e.name)) : 1), 0);
}

/** A function's name as the report carries it: name characters only.
 *  A quoted or computed method name can be any text — the report's own
 *  ` at ` delimiter, quotes, markup — and the scorer finds the file by
 *  where the name ends. */
function reportName(name) {
	return name.replace(/[^A-Za-z0-9_$#()!]/g, '_');
}

function xmlEscape(s) {
	return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function main(argv) {
	const tsv = argv.includes('--tsv');
	const roots = argv.filter((a) => a !== '--tsv');
	if (roots.length === 0) {
		console.error('usage: ts-ccn.mjs [--tsv] <path>...');
		return 2;
	}
	const found = { script: [], declaration: [], svelte: [], noScript: [], unknown: [], paraglide: 0 };
	for (const root of roots) {
		// A root that does not exist, or holds no script, would score the
		// language as empty; a mistyped path must not read as that.
		const before = found.script.length;
		try {
			collect(root, found);
		} catch (err) {
			console.error(`ts-ccn: ${root}: ${err.message}`);
			return 1;
		}
		if (found.script.length === before) {
			console.error(`ts-ccn: ${root}: no script files to measure`);
			return 1;
		}
	}
	if (found.unknown.length > 0) {
		for (const f of found.unknown) console.error(`ts-ccn: ${f}: neither measured nor declared — measure its kind or declare it in tools/ts-ccn.mjs`);
		return 1;
	}
	const svelte = found.svelte;
	const measured = [];
	let outside = 0;
	let failed = false;
	for (const file of found.script) {
		const result = measure(file, fs.readFileSync(file, 'utf-8'));
		const { units, errors } = result;
		outside += result.outside;
		if (errors.length > 0) {
			console.error(`ts-ccn: ${file}: does not parse: ${errors[0]}`);
			failed = true;
		}
		measured.push([file, units]);
	}
	if (failed) return 1;
	let out = '';
	if (tsv) {
		for (const [file, units] of measured) for (const u of units) out += `${file}\tfn\t${reportName(u.name)}\t${u.line}\t${u.endLine}\t${u.ccn}\n`;
	} else {
		out += '<?xml version="1.0" ?>\n<cppncss>\n\t<measure type="Function">\n';
		let nr = 0;
		for (const [file, units] of measured) {
			for (const u of units) {
				nr += 1;
				out += `\t\t<item name="${reportName(u.name)}(...) at ${xmlEscape(file)}:${u.line}">\n\t\t\t<value>${nr}</value>\n\t\t\t<value>${u.endLine + 1 - u.line}</value>\n\t\t\t<value>${u.ccn}</value>\n\t\t</item>\n`;
			}
		}
		out += '\t</measure>\n</cppncss>\n';
	}
	process.stdout.write(out);
	const fnCount = measured.reduce((n, [, units]) => n + units.length, 0);
	console.error(`ts-ccn: ${measured.length} script files, ${fnCount} functions measured`);
	if (outside > 0) {
		console.error(`ts-ccn: ${outside} decisions outside any function not counted (module-scope code)`);
	}
	if (svelte.length > 0) {
		console.error(`ts-ccn: not measured — ${svelte.length} .svelte components (script blocks are not TypeScript files, and coverage does not instrument them):`);
		for (const f of svelte) console.error(`  ${f}`);
	}
	const listed = [
		[found.declaration, 'declaration files (no function bodies)'],
		[found.noScript, 'files that hold no script (markup, styles, data)']
	];
	for (const [list, what] of listed) {
		if (list.length === 0) continue;
		console.error(`ts-ccn: not measured — ${list.length} ${what}:`);
		for (const f of list) console.error(`  ${f}`);
	}
	if (found.paraglide > 0) {
		console.error(`ts-ccn: skipped — ${found.paraglide} files of generated Paraglide output`);
	}
	return 0;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	process.exitCode = main(process.argv.slice(2));
}
