/**
 * The franchise page's titles with no clear place: line titles with
 * no number and no note, or a note leading into such a title. They
 * leave their lines and gather in one stretch at the end of the main
 * story, under its heading. Part of the franchise page's model.
 */
import type { FranchiseRow, FranchiseSection, RowContext } from './franchise-sections';
import { pathNote } from './order';

/**
 * The titles of `rows` with no clear place: no number, and either no
 * note or a note leading into a title with no clear place itself.
 */
function noClearPlace(ctx: RowContext, rows: readonly FranchiseRow[]): Set<string> {
	const leads = new Map<string, string>();
	for (const r of rows) {
		const note = pathNote(ctx.run, r.entry.id, ctx.order);
		if (note?.kind === 'leads-into') leads.set(r.entry.id, note.entry.id);
	}
	const unnumbered = rows.filter((r) => r.position === null).map((r) => r.entry.id);
	const loose = new Set(unnumbered.filter((id) => !leads.has(id)));
	for (let grew = true; grew; ) {
		grew = false;
		for (const id of unnumbered) {
			if (loose.has(id) || !loose.has(leads.get(id)!)) continue;
			loose.add(id);
			grew = true;
		}
	}
	return loose;
}

/**
 * `sections` with the titles that have no clear place gathered at the
 * end, and the lines left next to each other joined into one.
 */
export function gatherUnplaced(
	ctx: RowContext,
	sections: readonly FranchiseSection[]
): FranchiseSection[] {
	const lineRows = sections.flatMap((s) => (s.kind === 'line' ? s.rows : []));
	const loose = noClearPlace(ctx, lineRows);
	const placed = sections
		.map((s) =>
			s.kind === 'line' ? { ...s, rows: s.rows.filter((r) => !loose.has(r.entry.id)) } : s
		)
		.filter((s) => s.kind !== 'line' || s.rows.length > 0)
		.reduce<FranchiseSection[]>((out, s) => {
			// Numbered stretches left next to each other read as one list.
			const last = out.at(-1);
			if (s.kind === 'line' && last?.kind === 'line')
				out[out.length - 1] = { ...last, rows: [...last.rows, ...s.rows] };
			else out.push(s);
			return out;
		}, []);
	const rest = lineRows.filter((r) => loose.has(r.entry.id));
	return rest.length > 0 ? [...placed, { kind: 'line', rows: rest, unplaced: true }] : placed;
}
