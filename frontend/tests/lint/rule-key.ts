// How the design checks name a CSS rule: the stylesheet's path
// relative to `src/`, a space, then one selector with its whitespace
// collapsed. The lists the checks hold (exemptions, rules that must
// not set `opacity`) are written in this form, with forward slashes,
// so a Windows path's backslashes are turned into them here.

/** The key for `selector` in the stylesheet at `relPath`. */
export function ruleKey(relPath: string, selector: string): string {
	return `${relPath.replaceAll('\\', '/')} ${selector.replace(/\s+/g, ' ')}`;
}
