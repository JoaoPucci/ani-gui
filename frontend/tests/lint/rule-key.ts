// How the design checks name a CSS rule: the stylesheet's path
// relative to `src/`, a space, then one selector with its whitespace
// collapsed. The lists the checks hold (exemptions, rules that must
// not set `opacity`) are written in this form.

/** The key for `selector` in the stylesheet at `relPath`. */
export function ruleKey(relPath: string, selector: string): string {
	return `${relPath} ${selector.replace(/\s+/g, ' ')}`;
}
