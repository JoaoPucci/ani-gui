// The regression corpus for a Continue row's guess: rows that recorded
// no show, each with the show it is, and the pick the guess makes for
// it, pinned. Every case is a real provider listing — a hianime card's
// key and title — with the Kitsu answers its guess reads recorded in
// tests/fixtures/continue-guess/ (see its MANIFEST.json). A pick that
// changes here is a card that opens another show.
//
// The guess runs here as Continue runs it, from the row (`resolveHistoryEntry`)
// through `resolveKitsuMatchWithTrust`, with nothing stored: no mapping
// and no title match, so every case reaches the search. The hits the
// row rules out are the ones the case's `ruled_out` lists for the
// episode, which the backend's half of the corpus
// (backend/src/commands/continue_guess_corpus_test.rs) checks its rules
// against. The step after the search, the backend's resolve from the
// show key's words, answers what the case's `enrichment` records, which
// the backend's half checks too; a guess that reaches it for a case
// recording no answer fails, as does any request the recordings do not
// hold.

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { resolveKitsuMatchWithTrust } from './match';
import { resolveHistoryEntry } from './resolve';
import {
	allmangaKitsuMapGet,
	kitsuAnimeBySlug,
	kitsuAnimeDetail,
	kitsuGuessRuledOut,
	kitsuResolveAllmangaShowId,
	kitsuSearch,
	kitsuTitleMatchGet,
	kitsuTitleMatchPut,
	type KitsuAnimeRef
} from '$lib/api';

vi.mock('$lib/api', () => ({
	allmangaKitsuMapDelete: vi.fn(),
	allmangaKitsuMapGet: vi.fn(),
	allmangaKitsuMapPlayed: vi.fn(),
	kitsuAnimeBySlug: vi.fn(),
	kitsuAnimeDetail: vi.fn(),
	kitsuGuessRuledOut: vi.fn(),
	kitsuResolveAllmangaShowId: vi.fn(),
	kitsuSearch: vi.fn(),
	kitsuTitleMatchGet: vi.fn(),
	kitsuTitleMatchPut: vi.fn()
}));

interface Case {
	key: string;
	title: string;
	key_year: number | null;
	right: string | null;
	picks: Record<string, string | null>;
	ruled_out?: Record<string, string[]>;
	enrichment?: string | null;
	source: string;
}

const dir = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	'../../../../tests/fixtures/continue-guess'
);
const read = (name: string) => JSON.parse(readFileSync(path.join(dir, name), 'utf8'));
const cases: Case[] = read('cases.json');
const requests: { search: Record<string, string>; slug: Record<string, string> } =
	read('requests.json');

interface Resource {
	id: string;
	attributes: {
		canonicalTitle: string | null;
		titles: Record<string, string | null> | null;
		abbreviatedTitles: string[] | null;
		slug: string | null;
		startDate: string | null;
		endDate: string | null;
		episodeCount: number | null;
		subtype: string | null;
		status: string | null;
	};
}

/** A recorded page as the backend hands it on: music dropped, null
 *  titles and blank abbreviations left out. */
function hitsOf(file: string): KitsuAnimeRef[] {
	const body: { data: Resource[] } = read(file);
	return body.data
		.filter((r) => (r.attributes.subtype ?? '').toLowerCase() !== 'music')
		.map((r) => ({
			id: r.id,
			canonical_title: r.attributes.canonicalTitle ?? '',
			titles: Object.fromEntries(
				Object.entries(r.attributes.titles ?? {}).filter(
					(e): e is [string, string] => typeof e[1] === 'string'
				)
			),
			abbreviated_titles: (r.attributes.abbreviatedTitles ?? []).filter((s) => s.trim() !== ''),
			slug: r.attributes.slug,
			synopsis: null,
			start_date: r.attributes.startDate,
			end_date: r.attributes.endDate,
			episode_count: r.attributes.episodeCount,
			average_rating: null,
			subtype: r.attributes.subtype,
			status: r.attributes.status,
			age_rating: null,
			popularity_rank: null,
			poster_image: null,
			cover_image: null
		}));
}

/** Requests the guess made that the recording does not hold. */
let unrecorded: string[] = [];
let current: Case;
let currentEp: string;

beforeEach(() => {
	unrecorded = [];
	vi.mocked(allmangaKitsuMapGet).mockResolvedValue(null);
	vi.mocked(kitsuTitleMatchGet).mockResolvedValue(null);
	vi.mocked(kitsuTitleMatchPut).mockResolvedValue(undefined);
	vi.mocked(kitsuAnimeDetail).mockImplementation(async (id) => {
		unrecorded.push(`detail ${id}`);
		throw new Error('unrecorded');
	});
	vi.mocked(kitsuSearch).mockImplementation(async (q) => {
		const file = requests.search[q];
		if (!file) {
			unrecorded.push(`search ${q}`);
			throw new Error('unrecorded');
		}
		return hitsOf(file);
	});
	vi.mocked(kitsuAnimeBySlug).mockImplementation(async (slug) => {
		const file = requests.slug[slug];
		if (!file) {
			unrecorded.push(`slug ${slug}`);
			throw new Error('unrecorded');
		}
		return hitsOf(file)[0] ?? null;
	});
	// The case's `ruled_out` is the backend's answer for the search's and
	// the slug lookup's hits sent together as a search's answer, and a
	// lone hit is answered from it too. That matches the app as long as
	// no row whose search is refuted as a whole also looks a slug up, and
	// none does.
	vi.mocked(kitsuGuessRuledOut).mockImplementation(async (title, episode, hits, key) => {
		if (title !== current.title || episode !== parseFloat(currentEp) || key !== current.key) {
			unrecorded.push(`rules for ${key} ${title} at ${episode}`);
		}
		const out = new Set(current.ruled_out?.[currentEp] ?? []);
		return hits.filter((h) => out.has(h.id)).map((h) => h.id);
	});
	vi.mocked(kitsuResolveAllmangaShowId).mockImplementation(async (id) => {
		if (current.enrichment === undefined) {
			unrecorded.push(`enrichment ${id}`);
			return null;
		}
		return current.enrichment === null ? null : ({ id: current.enrichment } as KitsuAnimeRef);
	});
});

async function guess(c: Case, ep: string): Promise<string | null> {
	current = c;
	currentEp = ep;
	const row = resolveHistoryEntry(
		{ ep_no: ep, id: c.key, title: c.title, key_year: c.key_year ?? undefined },
		null
	);
	return (await resolveKitsuMatchWithTrust(row)).match?.id ?? null;
}

describe('the Continue guess over the corpus', () => {
	const runs = cases.flatMap((c) =>
		Object.keys(c.picks).map((ep) => [`${c.key} at episode ${ep}`, c, ep] as const)
	);

	it.each(runs)('%s', async (_name, c, ep) => {
		const picked = await guess(c, ep);
		expect(unrecorded).toEqual([]);
		expect(picked).toBe(c.picks[ep]);
	});

	it('holds the reported rows and a broad sample', () => {
		expect(cases.length).toBeGreaterThan(100);
		expect(cases.filter((c) => c.source === 'reported').length).toBeGreaterThan(2);
	});
});
