/**
 * The label a Kitsu subtype (`TV`, `movie`, `special`, `OVA`, `ONA`,
 * `music`) is shown with: in the search type chips, on result cards,
 * in the topbar's live results and on the detail page's eyebrow. A
 * missing subtype reads as TV, which is what Kitsu means by omitting
 * it.
 */
export function subtypeLabel(subtype: string | null | undefined): string {
	return (subtype ?? 'TV').toUpperCase();
}
