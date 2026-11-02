/**
 * A franchise's display name, from the title of the run's first entry:
 * Kitsu's format suffix ("(TV)", "(Movie)", …) is dropped, unless
 * another entry of the same franchise would end up with the same name,
 * in which case the suffix is what tells the two apart and stays.
 */

/** The title without a trailing Kitsu format suffix. */
export function withoutFormatSuffix(title: string): string {
	return title.replace(/\s*\((TV|Movie|OVA|ONA|Special)\)$/i, '');
}

/**
 * `title` without its format suffix, unless one of `others` (the rest
 * of the franchise's titles) reads the same once its own suffix is
 * dropped, ignoring case.
 */
export function franchiseName(title: string, others: readonly string[] = []): string {
	const bare = withoutFormatSuffix(title);
	const key = bare.toLowerCase();
	const clash = others.some((o) => withoutFormatSuffix(o).toLowerCase() === key);
	return clash ? title : bare;
}
