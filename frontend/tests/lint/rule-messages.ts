// What a lint test reads from the project's ESLint: the messages of the
// rules under test, and nothing it can mistake for a clean file.
//
// A test that keeps only its rules' messages would also drop the ones
// ESLint reports with no rule: a fixture that does not parse, a path an
// ignore pattern covers, a path no config block matches. Each means no
// rule read the file, and an "allows" case asserting `[]` would pass on
// it. So any message without a rule throws, naming the file and why.

import type { ESLint } from 'eslint';

/** The messages `rules` report in `result`, which ESLint produced for
 *  `filePath`. Throws if ESLint reported anything without a rule. */
export function ruleMessages(
	result: ESLint.LintResult,
	filePath: string,
	rules: string[]
): string[] {
	const unread = result.messages.filter((msg) => msg.ruleId === null);
	if (unread.length > 0) {
		throw new Error(`no rule read ${filePath}: ${unread.map((msg) => msg.message).join('; ')}`);
	}
	return result.messages
		.filter((msg) => msg.ruleId !== null && rules.includes(msg.ruleId))
		.map((msg) => msg.message);
}
