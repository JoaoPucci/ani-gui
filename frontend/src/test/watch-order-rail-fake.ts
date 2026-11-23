// A fake of the backend's Continue rail (`POST /api/watch-order/rail`,
// backend/src/watch_order/wire_next.rs `rail_view`), for the acceptance
// tests: Home reads its rail again whenever what it has read changes,
// so the requests a scenario sends depend on timing, and no record of
// answers could hold them all. The fake answers as the backend does;
// `watch-order-rail-fake.test.ts` holds it to the answers the backend
// recorded for every request the scenarios sent
// (tests/fixtures/watch-order/scenarios/*.json `rails`), which the
// backend's own scenario test checks against `rail_view`.

import type { AskRow, Handoff, RailRequest, RailView } from '../lib/api';
import { parseHistoryEpisode } from '../lib/play/episode-number';

/** `handoff_ask`: the Kitsu id a row's hand-off needs, or null. */
function ask(row: AskRow): string | null {
	const matched = row.matched;
	const episode = parseHistoryEpisode(row.ep_no);
	if (!matched || episode === null) return null;
	const count = matched.episode_count ?? null;
	const last = count === null ? null : Math.max(count, row.playable_count ?? 0);
	const finale = matched.status === 'finished' && last !== null && episode >= last;
	return finale && !row.left_part_way ? matched.id : null;
}

/** `played_ids`: null while any history row is still being matched. */
function played(req: RailRequest): Set<string> | null {
	const out = new Set<string>();
	for (const row of req.history) {
		if (row.kitsu_id != null) out.add(row.kitsu_id);
		if (!(row.id in req.matches)) return null;
		const found = req.matches[row.id];
		if (found !== null) out.add(found);
	}
	return out;
}

/** `rail_view`. */
export function railView(req: RailRequest): RailView {
	const asks: Record<string, string | null> = {};
	const order: [string, string | null][] = req.rows.map((r) => [r.row_id, ask(r)]);
	for (const [rowId, kitsuId] of order) asks[rowId] = kitsuId;
	const handoffs: Record<string, Handoff> = {};
	const ids = played(req);
	if (ids === null) return { asks, handoffs };
	const shown = new Set<string>();
	for (const [rowId, kitsuId] of order) {
		const handoff = kitsuId === null ? null : (req.series[kitsuId] ?? null);
		if (!handoff || ids.has(handoff.next.id) || shown.has(handoff.next.id)) continue;
		shown.add(handoff.next.id);
		handoffs[rowId] = handoff;
	}
	return { asks, handoffs };
}
