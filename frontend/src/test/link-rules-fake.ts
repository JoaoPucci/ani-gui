// A fake of the backend's change-show rules, for the renderer's tests:
// the unit tests' fake app dependencies and the acceptance tests' fake
// routes answer as the backend does, so a scenario reads the same as it
// would against the app.
//
// Each answers as its Rust counterpart, whose own tests hold the rule:
// - `verdict`: commands/link_verdict.rs `verdict`
// - `groupMembers`: commands/link_verdict.rs `group_members`
// - `cardLines`: commands/card_line.rs `card_lines`
// - `candidates`: commands/link_candidates.rs and link_rules.rs. A
//   title's season is read here with the renderer's own reader, which
//   the backend's mirrors on titles; a slug's as the backend reads it.

import type {
	CandidatesRequest,
	CardLine,
	CardLineRow,
	KitsuAnimeRef,
	LinkCandidate,
	LinkGroupRequest,
	LinkVerdict,
	LinkWarning,
	ReadOptions,
	RowMatch,
	VerdictRequest
} from '../lib/api';
import { parseCour } from '../lib/history/resolve';

export function verdict(r: VerdictRequest): LinkVerdict {
	const showsPage = r.show === r.page;
	const carried = r.carried && !r.picked;
	const guess = r.asks_guess && showsPage && (!r.trusted || (carried && !r.named));
	return {
		offer: guess ? 'guess' : showsPage && (r.picked || r.named) ? 'again' : null,
		kept: r.named && r.show !== null && !showsPage
	};
}

export function groupMembers(
	card: string,
	show: string,
	saved: string | null,
	rows: RowMatch[]
): string[] {
	const covered = (r: RowMatch) =>
		saved === null ? r.show === show && !r.trusted : r.show === saved;
	return [card, ...rows.filter((r) => r.id !== card && covered(r)).map((r) => r.id)];
}

export function cardLines(rows: CardLineRow[]): Record<string, CardLine> {
	const lineOf = (shown: CardLineRow): CardLineRow => {
		const from = shown.guessed_from;
		if (!from || from === shown.id || !shown.guess || shown.show === null) return shown;
		return rows.find((r) => r.id === from && r.show === shown.show) ?? shown;
	};
	return Object.fromEntries(
		rows.map((shown) => {
			const line = lineOf(shown);
			return [shown.id, { line: line.id, correctable: line.guess || line.named }];
		})
	);
}

const isMusic = (ref: KitsuAnimeRef) => (ref.subtype ?? '').toLowerCase() === 'music';

export function mergeCandidates(
	current: KitsuAnimeRef | null,
	titleHits: KitsuAnimeRef[],
	slugHits: KitsuAnimeRef[]
): [KitsuAnimeRef, boolean][] {
	const ordered: KitsuAnimeRef[] = current ? [current] : [];
	for (let i = 0; i < Math.max(titleHits.length, slugHits.length); i++) {
		if (titleHits[i]) ordered.push(titleHits[i]);
		if (slugHits[i]) ordered.push(slugHits[i]);
	}
	const seen = new Set<string>();
	return ordered
		.filter((ref) => !isMusic(ref) && !seen.has(ref.id) && seen.add(ref.id))
		.map((ref) => [ref, ref.id === current?.id]);
}

const ORDINALS = [
	'first',
	'second',
	'third',
	'fourth',
	'fifth',
	'sixth',
	'seventh',
	'eighth',
	'ninth',
	'tenth'
];

/** A slug's season as the backend reads it (`cour_from_slug`): a
 *  trailing `-part-N` / `-cour-N` / `-season-N`, or an ordinal before
 *  the keyword, spelled, with a suffix, or a bare number. */
/** A number the backend holds as a season (a u32), else none. */
const held = (n: number): number | null => (n <= 4_294_967_295 ? n : null);

function slugSeason(slug: string): number | null {
	const keyword = slug.match(/(?:^|-)(?:part|cour|season)-(\d+)$/i);
	if (keyword) return held(Number(keyword[1]));
	const ordinal = slug.match(/(?:^|-)([a-z0-9]+)-(?:part|cour|season)$/i)?.[1]?.toLowerCase();
	if (!ordinal) return null;
	const spelled = ORDINALS.indexOf(ordinal);
	if (spelled >= 0) return spelled + 1;
	const digits = ordinal.replace(/(?:st|nd|rd|th)$/, '');
	return /^\d+$/.test(digits) ? held(Number(digits)) : null;
}

function seasonOf(ref: KitsuAnimeRef): number {
	const titles = [ref.canonical_title, ...Object.values(ref.titles ?? {})].map((n) => parseCour(n));
	const slug = ref.slug ? slugSeason(ref.slug) : null;
	return Math.max(1, ...titles, ...(slug === null ? [] : [slug]));
}

export function linkWarnings(title: string, ref: KitsuAnimeRef, episode: number): LinkWarning[] {
	const warnings: LinkWarning[] = [];
	if (parseCour(title) !== seasonOf(ref)) warnings.push('season');
	if (ref.episode_count !== null && episode > ref.episode_count) warnings.push('episode');
	return warnings;
}

/** The words the backend reads off a row's key (`key_search_term`). */
export function keyTerm(id: string): string | null {
	const words = id.replace(/^[a-z]+:/, '').match(/^(.+)-\d+$/)?.[1] ?? null;
	return words !== null && /^[a-z0-9-]+$/.test(words)
		? words.split('-').filter(Boolean).join(' ')
		: null;
}

/** The backend's candidates, its searches answered by `search`. */
export async function candidates(
	r: CandidatesRequest,
	search: (query: string, opts?: ReadOptions) => Promise<KitsuAnimeRef[]>,
	opts?: ReadOptions
): Promise<LinkCandidate[]> {
	let shows: [KitsuAnimeRef, boolean][];
	if (r.query !== null) {
		const hits = await search(r.query.trim(), opts);
		shows = mergeCandidates(null, hits, []).map(([ref]) => [ref, ref.id === r.current?.id]);
	} else {
		const quiet = (q: string | null) =>
			q && q.trim() ? search(q.trim(), opts).catch(() => []) : Promise.resolve([]);
		const [title, slug] = await Promise.all([quiet(r.title), quiet(keyTerm(r.row))]);
		shows = mergeCandidates(r.current, title, slug);
	}
	return shows.map(([ref, current]) => ({
		ref,
		current,
		warnings: linkWarnings(r.title, ref, r.episode)
	}));
}

/** The backend's rules as a link session's dependencies, its searches
 *  answered by `search`: the candidates, the verdict, and the group, of
 *  which every row shares the card's listing. */
export function ruleDeps(search: (query: string, opts?: ReadOptions) => Promise<KitsuAnimeRef[]>): {
	candidates: (r: CandidatesRequest, opts?: ReadOptions) => Promise<LinkCandidate[]>;
	verdict: (r: VerdictRequest) => Promise<LinkVerdict>;
	group: (r: LinkGroupRequest) => Promise<string[]>;
} {
	return {
		candidates: (r, opts) => candidates(r, search, opts),
		verdict: async (r) => verdict(r),
		group: async (r) => groupMembers(r.card, r.show, r.saved, r.rows)
	};
}
