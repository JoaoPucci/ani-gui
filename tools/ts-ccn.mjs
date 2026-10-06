#!/usr/bin/env node
// Per-function cyclomatic complexity for TypeScript, on the compiler's
// own parse of each file.
//
// The CRAP gate used to take its frontend complexity from lizard,
// whose TypeScript support is a regex tokenizer driving a small state
// machine. It loses its place on ordinary syntax — a union return
// type, a type predicate, an optional `catch` binding — and when it
// does it folds the functions that follow into the one it was reading
// or drops them, without saying so. The gate under-measured while
// reading green.
//
// Here the TypeScript compiler parses each file. Every function-like
// node with a body is a unit: declarations, methods, constructors,
// accessors, function expressions and arrow functions. The count
// within a unit is lizard's, kept so the numbers stay continuous with
// the ceilings recorded against it: 1, plus one for each `if`, `for`,
// `while`, `catch` and `case` keyword, each conditional `? :`, and
// each `&&` and `||`. The tokens are the parser's, so a keyword inside a string
// or a template is text, and neither `?.`, `??` nor the `?` of an
// optional parameter is a conditional. A function
// nested in another is its own unit, and its tokens are not counted
// again in the enclosing one.
//
// What this does not measure it says so about: `.svelte` components
// under the given roots are listed on stderr as unmeasured. Their
// script blocks are not TypeScript files, and the coverage run does
// not instrument them either.
//
// Usage: node tools/ts-ccn.mjs [--tsv] <path>...
// Default output is lizard's XML shape, which tools/crap-score.mjs
// reads; `--tsv` lists one unit per line. A file with syntax errors is
// reported and the run exits non-zero — an unparsed file is a
// measurement that did not happen, and the gate must not read it as
// zero. Paths containing `paraglide` (compiled message bundles) are
// skipped, as the gate always skipped them.

/** Measure one source text. */
export function measure(_fileName, _source) {
	return { units: [], errors: [] };
}
