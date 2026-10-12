/**
 * Custom ESLint rule: every visitor-facing string in a Svelte component,
 * or in a TypeScript module that feeds one, must be routed through
 * Paraglide (`m.foo()`), not embedded as a literal. Catches three slip
 * surfaces:
 *
 *   1. Raw text in a template — `<button>Save</button>`. Flagged via
 *      the `SvelteText` AST node.
 *   2. Literal attribute values on user-facing attributes —
 *      `aria-label="Close"`, `placeholder="Search"`, `title="…"`,
 *      `alt="…"`. Flagged via `SvelteAttribute` whose first child is
 *      a `SvelteLiteral`.
 *   3. String and template literals in JavaScript — in `<script>`, in
 *      template expressions and in `.ts` modules — that read as copy. A literal in
 *      script reaches the screen through a variable (`{error.headline}`,
 *      a `{#each}` over a label table, `window.confirm(…)`), which the
 *      first two never see.
 *
 * Heuristic for 1 and 2: a value is "translatable" when it has ≥2
 * ASCII letters and either contains a space, ends with sentence
 * punctuation or starts with a capital. Pure numbers, lowercase single
 * tokens, IDs, hex colours, URL fragments, mono-character glyphs
 * (✓, ▾, ▸…) are skipped.
 *
 * Heuristic for 3, stricter because script is full of strings that are
 * not copy: the literal starts with a capitalised word ("Best", "All
 * files", "Couldn't reach Kitsu.") or contains a space and ends with
 * sentence punctuation ("HLS playback is not supported."). Literals in
 * positions that are never copy are skipped: import sources, object
 * keys and member names, comparison operands, `case` labels, type
 * positions, `console.*` arguments, `new …Error(…)` messages, tagged
 * templates, and expressions on template attributes that are not
 * user-facing (`class`, `href`, directives).
 *
 * What 3 deliberately misses — say so rather than imply coverage:
 *   - a literal that starts lowercase or with an all-caps word and has
 *     no closing punctuation ("HLS fatal: …", "EP 3", "movie")
 *   - copy built by joining non-copy fragments (`${a} ${b}`)
 *   - any literal inside a template attribute's expression when the
 *     attribute is not one of USER_FACING_ATTRS — which includes event
 *     handlers (`onclick={() => confirm('…')}`) and component props
 *     (`<ErrorOverlay body={'…'} />`)
 *   - files eslint.config.js does not hand it: it runs on `.svelte`,
 *     `.svelte.ts` and `.svelte.js` files and on every `.ts` module
 *     under src/ except tests, so plain `.js` modules and anything
 *     outside src/ go unchecked
 * Copy in those places is caught by review, not by this rule. It also
 * over-reports in two known spots, both cheap to silence: a `style:`
 * directive's value, and an Error message built without `new`.
 *
 * Escape hatch: a comment containing `i18n-ignore` silences the rule
 * for a node. In a template, a Svelte comment just before the node; in
 * JavaScript, a comment on the line before the literal or on its line,
 * or within the eight tokens before it. The marker covers every
 * literal on the line it applies to, so keep one literal per marked
 * line.
 *
 * The rule is intentionally noisy on suspicion: false positives are
 * cheap (one-line ignore comment) but a missed translation slips
 * past every check we have.
 */

const USER_FACING_ATTRS = new Set([
	'aria-label',
	'aria-description',
	'aria-roledescription',
	'aria-placeholder',
	'aria-valuetext',
	'title',
	'placeholder',
	'alt',
	'label'
]);

/** Common safe singletons that look capital but are tokens. */
const TOKEN_ALLOWLIST = new Set([
	'EN',
	'JP',
	'OK',
	'CC',
	'UTC',
	'AM',
	'PM',
	'TV',
	'MP4',
	'HLS',
	'PiP',
	'OP',
	'ED',
	'ETH'
]);

/** Cheap heuristic for "looks like English text we'd want translated".
 *  @param {unknown} raw */
function isTranslatable(raw) {
	if (typeof raw !== 'string') return false;
	const value = raw.trim();
	if (value.length < 2) return false;
	// Single-character glyphs / dingbats — common as decorative button content.
	if (value.length === 1) return false;
	// All-numeric, all-punct, ID-like (kebab/snake/camel without spaces).
	const letters = (value.match(/[A-Za-z]/g) ?? []).length;
	if (letters < 2) return false;
	// Pure tokens with no separator are usually identifiers / class names /
	// CSS values — not translatable copy.
	const hasSpace = /\s/.test(value);
	const endsSentence = /[.!?…]$/.test(value);
	const startsCapital = /^[A-Z]/.test(value);
	if (!hasSpace && !endsSentence && !startsCapital) return false;
	if (TOKEN_ALLOWLIST.has(value)) return false;
	return true;
}

/** The stricter check for a JavaScript literal (surface 3 above).
 *  @param {string} raw */
function isScriptCopy(raw) {
	const value = raw.trim();
	if ((value.match(/[A-Za-z]/g) ?? []).length < 2) return false;
	if (TOKEN_ALLOWLIST.has(value)) return false;
	if (/^[A-Z][a-z]/.test(value)) return true;
	return /\s/.test(value) && /[.!?…]$/.test(value);
}

/** Error constructors whose message argument is never copy. */
const ERROR_CALLEE = /(^|\.)([A-Z][A-Za-z]*)?Error$/;

/** Is this JavaScript literal in a position that is never copy?
 *  @param {any} node
 *  @param {any} sourceCode */
function isNonCopyPosition(node, sourceCode) {
	const parent = node.parent;
	if (!parent) return true;
	switch (parent.type) {
		case 'ImportDeclaration':
		case 'ImportExpression':
		case 'ExportNamedDeclaration':
		case 'ExportAllDeclaration':
		case 'TSLiteralType':
		case 'TaggedTemplateExpression':
		case 'SwitchCase':
			return true;
		case 'Property':
			if (parent.key === node) return true;
			break;
		case 'MemberExpression':
			if (parent.property === node) return true;
			break;
		case 'BinaryExpression':
			if (['===', '!==', '==', '!='].includes(parent.operator)) return true;
			break;
		case 'CallExpression':
			if (/^console\./.test(sourceCode.getText(parent.callee))) return true;
			break;
		case 'NewExpression':
			if (ERROR_CALLEE.test(sourceCode.getText(parent.callee))) return true;
			break;
	}
	// Inside a template attribute's expression: only user-facing
	// attributes carry copy; `class={…}`, `href={…}` and directives
	// (`on:`, `bind:`, `class:` …) do not.
	for (let cur = parent; cur; cur = cur.parent) {
		if (cur.type === 'SvelteDirective' || cur.type === 'SvelteSpecialDirective') return true;
		if (cur.type === 'SvelteAttribute') {
			const name = typeof cur.key?.name === 'string' ? cur.key.name : '';
			return !USER_FACING_ATTRS.has(name);
		}
		if (cur.type === 'SvelteScriptElement' || cur.type === 'Program') break;
	}
	return false;
}

/** Does an `i18n-ignore` comment sit on the line before `node` or on
 *  its own line?
 *  @param {any} node
 *  @param {any} sourceCode */
function hasLineIgnoreMarker(node, sourceCode) {
	const line = node.loc.start.line;
	return sourceCode
		.getAllComments()
		.some(
			(/** @type {any} */ c) =>
				c.value.includes('i18n-ignore') &&
				(c.loc.end.line === line - 1 || c.loc.start.line === line)
		);
}

/** Walk up the AST to find a parent of `type`.
 *  @param {any} node
 *  @param {string} type */
function findParent(node, type) {
	let cur = node.parent;
	while (cur) {
		if (cur.type === type) return cur;
		cur = cur.parent;
	}
	return null;
}

/** Is the node inside a Svelte comment-marked block? Walks back a few
 *  tokens looking for `i18n-ignore` in the previous comment.
 *  @param {any} node
 *  @param {any} sourceCode */
function hasIgnoreMarker(node, sourceCode) {
	const before = sourceCode.getTokensBefore(node, { count: 8, includeComments: true });
	for (const t of before) {
		if (t.type === 'HTMLComment' && t.value && t.value.includes('i18n-ignore')) return true;
		if (t.type === 'Block' && t.value && t.value.includes('i18n-ignore')) return true;
		if (t.type === 'Line' && t.value && t.value.includes('i18n-ignore')) return true;
	}
	return false;
}

/** @type {import('eslint').Rule.RuleModule} */
export default {
	meta: {
		type: 'problem',
		docs: {
			description:
				'disallow hardcoded English text in Svelte components — route every visitor-facing string through Paraglide (m.foo())'
		},
		schema: [],
		messages: {
			text: 'Hardcoded text "{{ value }}" — wrap in Paraglide (m.foo()) or annotate with <!-- i18n-ignore -->.',
			attr: 'Hardcoded text in `{{ attr }}` ("{{ value }}") — wrap in Paraglide (m.foo()) or annotate with <!-- i18n-ignore -->.',
			script:
				'Hardcoded text "{{ value }}" in script — wrap in Paraglide (m.foo()) or annotate with // i18n-ignore.'
		}
	},
	create(context) {
		const sourceCode = context.sourceCode;

		/** @param {any} node
		 *  @param {string} value */
		function checkScriptLiteral(node, value) {
			if (!isScriptCopy(value)) return;
			if (isNonCopyPosition(node, sourceCode)) return;
			if (hasLineIgnoreMarker(node, sourceCode) || hasIgnoreMarker(node, sourceCode)) return;
			context.report({
				node,
				messageId: 'script',
				data: { value: value.trim().slice(0, 50) }
			});
		}

		return {
			/** @param {any} node */
			Literal(node) {
				if (typeof node.value === 'string') checkScriptLiteral(node, node.value);
			},
			/** @param {any} node */
			TemplateLiteral(node) {
				const text = node.quasis.map((/** @type {any} */ q) => q.value.cooked ?? '').join('{}');
				checkScriptLiteral(node, text);
			},
			/** @param {any} node */
			SvelteText(node) {
				if (!isTranslatable(node.value)) return;
				if (hasIgnoreMarker(node, sourceCode)) return;
				// Inside a <style>, <script>, or {@const} block — not user-visible.
				if (findParent(node, 'SvelteStyleElement')) return;
				if (findParent(node, 'SvelteScriptElement')) return;
				context.report({
					node,
					messageId: 'text',
					data: { value: node.value.trim().slice(0, 50) }
				});
			},
			/** @param {any} node */
			SvelteAttribute(node) {
				const attrName = typeof node.key?.name === 'string' ? node.key.name : null;
				if (!attrName || !USER_FACING_ATTRS.has(attrName)) return;
				if (!Array.isArray(node.value) || node.value.length !== 1) return;
				const child = node.value[0];
				if (!child || child.type !== 'SvelteLiteral') return;
				if (!isTranslatable(child.value)) return;
				if (hasIgnoreMarker(node, sourceCode)) return;
				context.report({
					node,
					messageId: 'attr',
					data: {
						attr: attrName,
						value: String(child.value).trim().slice(0, 50)
					}
				});
			}
		};
	}
};
