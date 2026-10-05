// Step 0 of a Continue row's Kitsu resolution: the mapping stored for
// the row's show id, split from match.ts so its sequencing stays
// readable and inside the complexity ratchet.

import {
	allmangaKitsuMapDelete,
	allmangaKitsuMapGet,
	allmangaKitsuMapPlayed,
	kitsuAnimeDetail,
	type KitsuAnimeRef
} from '$lib/api';
import { cachedBindingVerdict, onlyTitleInDoubt, type ResumeTarget } from './resolve';

/** The stored mapping for the row's show id when it can stand as the
 *  answer, and whether a real play stored it, else null and the row
 *  resolves on. Every endpoint failure is the same as finding nothing,
 *  and a played read that fails is the same as no play. */
export async function storedBinding(
	preliminary: ResumeTarget
): Promise<{ ref: KitsuAnimeRef; played: boolean } | null> {
	if (!preliminary.allmangaShowId) return null;
	let cached: KitsuAnimeRef;
	try {
		const kitsuId = await allmangaKitsuMapGet(preliminary.allmangaShowId);
		if (!kitsuId) return null;
		cached = await kitsuAnimeDetail(kitsuId);
	} catch {
		// The endpoint is down or the stored id is stale.
		return null;
	}
	const verdict = cachedBindingVerdict(cached, preliminary, true);
	const played = () => allmangaKitsuMapPlayed(preliminary.allmangaShowId).catch(() => false);
	if (verdict === 'trust') return { ref: cached, played: await played() };
	if (verdict === 'evict') {
		// Provably wrong (a music entry). Awaited: the enrichment step
		// reads this same reverse cache first, so the delete must commit
		// before the row falls through to it. A failing delete is
		// tolerated.
		await allmangaKitsuMapDelete(preliminary.allmangaShowId).catch(() => {});
		return null;
	}
	// A provider title Kitsu does not use doubts the right binding on
	// its title alone ("There Is Also a Hole in the Student
	// Organization!" for Seitokai ni mo Ana wa Aru!). The binding a play
	// stored is the show the user played, so it stands; a guess an
	// earlier resolve stored does not.
	if (!onlyTitleInDoubt(cached, preliminary, true)) return null;
	return (await played()) ? { ref: cached, played: true } : null;
}
