// The renderer's types for what crosses the HTTP seam are written by
// hand; nothing generates them from the backend's structs. This test is
// what keeps them honest. The backend publishes its side as
// `backend/wire-contract.json` (built from its types, and held to them
// by `cargo test`: see `backend/src/api/wire_contract_test.rs`), and
// each renderer type is held to it here through a spec the compiler
// derives from the type itself, so neither side can change alone.
//
// For a response type the spec names each field the type declares,
// whether it is optional, and the JSON kinds its value can take
// (`'string|null'`, `'?number'`, `'object[]'`); the compiler rejects a
// spec that does not say exactly what the type says. The test then
// reads the backend's samples: a field the backend sends and the type
// lacks, a field the type declares and the backend never sends, a field
// always sent that the type marks optional (or the reverse), and a kind
// one side has and the other does not, each fail with the field named.
//
// What the check cannot see, it says so:
// - It reads kinds, not values: a string field's allowed strings are
//   held only where the renderer names them as a union, through the
//   enum lists at the end, and the error kinds it compares against.
// - A nested object field is held as an object; the struct it holds
//   is held by its own entry. A map's values (`titles`, `cached`,
//   `playable_episode_counts`) and the elements of an array of arrays
//   are not held at all, and a field typed `any` would pass any spec.
// - Which route answers which type is not held: each entry names its
//   renderer type by hand. Answers that are not a struct (a map such
//   as `/api/watched-at`, a bare string, a list of strings, and
//   whether a route may answer `null`) are not in the contract.
// - The backend's minimal samples are taken as written: a field set
//   in one would read here as always sent.
// - Request bodies are held by field name and by which fields the
//   backend requires, not by the kinds of their values, and a body
//   nested in another (the guess's `hits`) is not held.
// - Query strings and URL paths the renderer builds as text are not
//   held, but for the proxy media URL below; nor is the search-page
//   body, which the backend reads through structs local to its reader.

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';

import {
	MEDIA_KINDS,
	VERSION_KINDS,
	type AiringStatus,
	type AniErrorPayload,
	type AppInfo,
	type AvailabilityArgs,
	type AvailabilityBatchBody,
	type AvailabilityBatchResponse,
	type AvailabilityResponse,
	type AvailabilityWarmBody,
	type Config,
	type CreateSessionArgs,
	type CreateSessionResponse,
	type DownloadArgs,
	type DownloadProgress,
	type DownloadResponse,
	type ExternalPlayerKind,
	type GuessRuledOutBody,
	type HistoryEntry,
	type HistoryNumber,
	type KitsuAnimeRef,
	type KitsuByMalIdsBody,
	type KitsuCoverImage,
	type KitsuEpisode,
	type KitsuEpisodeThumbnail,
	type KitsuPosterImage,
	type KitsuSearchBody,
	type KitsuSearchPage,
	type LaunchExternalPlayerArgs,
	type PlayArgs,
	type PlayProgress,
	type RefillsBody,
	type SearchOrder,
	type SessionSubtitle,
	type SkipInterval,
	type TitleMatchBody,
	type UnsettledReason,
	type UpcomingEpisode,
	type VersionInfo,
	type historyByKitsu
} from './api';
import type { RetiredSweepReport } from './diagnostics/retired-sweep';
import type { CacheReclaimReport } from './diagnostics/cache-reclaim';
import type { RefillEnded } from './refill/refill-waiter';
import type { StreamProvider } from './history/show-key';
import type { ReleaseInfo } from './update/release-parse';
import type {
	EntryView,
	ListEntry,
	ListStatus,
	PkceWire,
	Provider,
	Tokens,
	UserProfile,
	UserStats
} from './account/types';
import type {
	AuthUrlRequest,
	AuthUrlResponse,
	ExchangeCodeRequest,
	RefreshBody,
	UpdateProgressBody
} from './account/api';
import type { SetEntryBody } from './account/entry-api';
import { WATCH_LATER_BRIDGE_MAX_IDS } from './account/watch-later-loader';
import { buildMediaUrl } from './play/media-url';
import type { SidecarTrack } from './play/sidecar-tracks';

// — The spec a type implies ————————————————————————————————————————

/** The JSON kind of a value of type `V`, per member of a union. */
type Kind<V> = V extends string
	? 'string'
	: V extends number
		? 'number'
		: V extends boolean
			? 'boolean'
			: V extends null
				? 'null'
				: V extends readonly (infer E)[]
					? `${ElementKind<E>}[]`
					: V extends object
						? 'object'
						: never;

type ElementKind<E> = E extends string
	? 'string'
	: E extends number
		? 'number'
		: E extends boolean
			? 'boolean'
			: E extends null
				? 'null'
				: E extends readonly unknown[]
					? 'array'
					: E extends object
						? 'object'
						: never;

/** Every ordering of a union of kinds, joined with `|`. */
type Joined<U extends string, All extends string = U> = [U] extends [never]
	? never
	: U extends unknown
		? [Exclude<All, U>] extends [never]
			? U
			: `${U}|${Joined<Exclude<All, U>>}`
		: never;

type IsOptional<T, K extends keyof T> = object extends Pick<T, K> ? true : false;

/** What a response type says of one field: `?` when optional, then the
 *  kinds its value can take. */
type FieldSpec<
	T,
	K extends keyof T
> = `${IsOptional<T, K> extends true ? '?' : ''}${Joined<Kind<Exclude<T[K], undefined>>>}`;

/** Everything a response type says, field by field. */
type Spec<T> = { [K in keyof T]-?: FieldSpec<T, K> };

/** Everything a request type says: which fields it always sends. */
type BodySpec<T> = { [K in keyof T]-?: IsOptional<T, K> extends true ? 'optional' : 'required' };

type Equal<A, B> =
	(<X>() => X extends A ? 1 : 2) extends <X>() => X extends B ? 1 : 2 ? true : false;

/** Compiles only when `A` and `B` are the same type. */
function same<A, B>(proof: Equal<A, B>): void {
	void proof;
}

// — The backend's side ——————————————————————————————————————————————

type Json = Record<string, unknown>;

interface Contract {
	responses: Record<string, { full: Json; min: Json }>;
	requests: Record<string, { accepts: string[]; requires: string[] }>;
	enums: Record<string, string[]>;
	errors: Json[];
	constants: Record<string, number>;
	media_urls: { base: string; session: string; urls: Record<string, string> };
}

const CONTRACT: Contract = JSON.parse(
	readFileSync(new URL('../../../backend/wire-contract.json', import.meta.url), 'utf-8')
);

// — The checks ——————————————————————————————————————————————————————

function elementKind(v: unknown): string {
	if (v === null) return 'null';
	if (Array.isArray(v)) return 'array';
	return typeof v === 'object' ? 'object' : typeof v;
}

/** The kinds one JSON value shows. An empty array shows none. */
function kindsOf(v: unknown): string[] {
	if (Array.isArray(v)) return v.map((e) => `${elementKind(e)}[]`);
	return [elementKind(v)];
}

const sorted = (xs: Iterable<string>) => [...new Set(xs)].sort();

/**
 * Where a type's spec and the values the backend sends disagree, one
 * line per field. `samples` are what the backend sends; a field in
 * every sample is always sent. `optionalAnyway` names fields the type
 * may mark optional though the backend always sends them, for a type
 * that also describes values the renderer builds itself.
 */
function responseDrift(
	name: string,
	samples: readonly Json[],
	spec: Readonly<Record<string, string>>,
	optionalAnyway: readonly string[] = []
): string[] {
	const problems: string[] = [];
	const sent = sorted(samples.flatMap((s) => Object.keys(s)));
	for (const field of sent) {
		if (!(field in spec)) problems.push(`${name}.${field}: sent, but the type does not declare it`);
	}
	for (const [field, declared] of Object.entries(spec)) {
		const at = `${name}.${field}`;
		const present = samples.filter((s) => field in s);
		if (present.length === 0) {
			problems.push(`${at}: declared, but the backend never sends it`);
			continue;
		}
		const optional = declared.startsWith('?');
		const always = present.length === samples.length;
		if (always && optional && !optionalAnyway.includes(field)) {
			problems.push(`${at}: always sent, but the type marks it optional`);
		}
		if (!always && !optional) problems.push(`${at}: left out of some answers, but required`);
		const seen = sorted(present.flatMap((s) => kindsOf(s[field])));
		const said = sorted(declared.replace(/^\?/, '').split('|'));
		if (seen.join('|') !== said.join('|')) {
			problems.push(`${at}: the backend sends ${seen.join('|')}, the type says ${said.join('|')}`);
		}
	}
	return problems;
}

/**
 * Where a body's spec and the fields the backend reads disagree.
 * `unsent` names fields the backend reads that the type deliberately
 * never carries.
 */
function requestDrift(
	name: string,
	backend: { accepts: readonly string[]; requires: readonly string[] },
	spec: Readonly<Record<string, string>>,
	unsent: readonly string[] = []
): string[] {
	const problems: string[] = [];
	for (const field of Object.keys(spec)) {
		if (!backend.accepts.includes(field)) {
			problems.push(`${name}.${field}: sent, but the backend reads no such field`);
		}
	}
	for (const field of backend.requires) {
		if (spec[field] !== 'required') {
			problems.push(`${name}.${field}: the backend requires it, the type may leave it out`);
		}
	}
	for (const field of backend.accepts) {
		if (!(field in spec) && !unsent.includes(field)) {
			problems.push(`${name}.${field}: the backend reads it, the type does not declare it`);
		}
	}
	for (const field of unsent) {
		if (!backend.accepts.includes(field)) {
			problems.push(`${name}.${field}: listed as unsent, but the backend reads no such field`);
		}
	}
	return problems;
}

// — The renderer's side —————————————————————————————————————————————

type ProgressOf<K extends PlayProgress['kind']> = Extract<PlayProgress, { kind: K }>;

/** Each response the backend publishes, by its backend name, and the
 *  spec of the renderer type that reads it. */
const RESPONSES: Record<string, Record<string, string>> = {
	AppInfo: {
		version: 'string',
		history_path: 'string',
		proxy_base_url: 'string',
		removed_legacy_paths: 'string[]',
		retired_provider_sweep: 'object',
		cache_reclaim: 'object'
	} satisfies Spec<AppInfo>,
	RetiredSweepReport: {
		superseded: 'string[]',
		renumbered: 'string[]',
		numbering_rows: 'number',
		cache_rows: 'number'
	} satisfies Spec<RetiredSweepReport>,
	ReclaimReport: {
		undeclared_rows: 'number',
		expired_rows: 'number'
	} satisfies Spec<CacheReclaimReport>,
	HistoryRow: {
		ep_no: 'string',
		id: 'string',
		title: 'string',
		watched_at: '?number',
		kitsu_id: '?string',
		key_year: '?number'
	} satisfies Spec<HistoryEntry>,
	HistoryEntry: {
		ep_no: 'string',
		id: 'string',
		title: 'string',
		watched_at: '?number',
		kitsu_id: '?string'
	} satisfies Spec<NonNullable<Awaited<ReturnType<typeof historyByKitsu>>>>,
	HistoryNumber: {
		ep_no: 'string',
		former_ep_no: '?string'
	} satisfies Spec<HistoryNumber>,
	CreateSessionResponse: {
		session_id: 'string',
		media_url: 'string',
		media_kind: 'string',
		cache_hit: 'boolean',
		subtitles: 'object[]',
		episode: '?string',
		former_episode: '?string',
		episode_unverified: '?boolean',
		versions: '?object[]',
		version: '?string',
		own_listing: '?boolean'
	} satisfies Spec<CreateSessionResponse>,
	SessionSubtitle: {
		lang: 'string',
		label: 'string',
		default: 'boolean',
		url: 'string'
	} satisfies Spec<SessionSubtitle>,
	VersionInfo: {
		kind: 'string',
		show_key: 'string',
		has_mode: '?boolean'
	} satisfies Spec<VersionInfo>,
	KitsuAnimeRef: {
		id: 'string',
		canonical_title: 'string',
		titles: 'object',
		abbreviated_titles: 'string[]',
		slug: 'string|null',
		synopsis: 'string|null',
		start_date: 'string|null',
		end_date: 'string|null',
		episode_count: 'number|null',
		average_rating: 'number|null',
		subtype: 'string|null',
		status: 'string|null',
		age_rating: 'string|null',
		popularity_rank: 'number|null',
		poster_image: 'object|null',
		cover_image: 'object|null',
		anilist_banner: '?string'
	} satisfies Spec<KitsuAnimeRef>,
	KitsuPosterImage: {
		tiny: 'string|null',
		small: 'string|null',
		medium: 'string|null',
		large: 'string|null',
		original: 'string|null'
	} satisfies Spec<KitsuPosterImage>,
	KitsuCoverImage: {
		tiny: 'string|null',
		small: 'string|null',
		large: 'string|null',
		original: 'string|null'
	} satisfies Spec<KitsuCoverImage>,
	KitsuEpisode: {
		id: 'string',
		canonical_title: 'string|null',
		season_number: 'number|null',
		number: 'number|null',
		relative_number: 'number|null',
		length: 'number|null',
		synopsis: 'string|null',
		airdate: 'string|null',
		thumbnail: 'object|null'
	} satisfies Spec<KitsuEpisode>,
	KitsuEpisodeThumbnail: {
		original: 'string|null'
	} satisfies Spec<KitsuEpisodeThumbnail>,
	KitsuSearchPage: {
		hits: 'object[]',
		next_offset: 'number|null'
	} satisfies Spec<KitsuSearchPage>,
	SkipInterval: {
		skip_type: 'string',
		start_time: 'number',
		end_time: 'number'
	} satisfies Spec<SkipInterval>,
	AiringStatus: {
		aired: 'number|null',
		next_episode: 'number|null',
		next_airing_at: 'number|null',
		upcoming: 'object[]'
	} satisfies Spec<AiringStatus>,
	UpcomingEpisode: {
		episode: 'number',
		airing_at: 'number'
	} satisfies Spec<UpcomingEpisode>,
	Config: {
		locale: 'string',
		mode: 'string',
		download_quality: 'string',
		external_player: 'string',
		external_player_kind: 'string',
		external_player_custom_args: 'string',
		syncplay_binary: 'string',
		image_cache_cap_mb: 'number',
		auto_play_next: 'boolean',
		cache_resolutions: 'boolean',
		download_bottom_bar_enabled: 'boolean',
		auto_skip_op: 'boolean',
		auto_skip_ed: 'boolean',
		use_custom_player_controls: 'boolean',
		update_include_prereleases: 'boolean',
		primary_account: 'string'
	} satisfies Spec<Config>,
	'ProgressLine.searching': {
		kind: 'string',
		provider: 'string'
	} satisfies Spec<ProgressOf<'searching'>>,
	'ProgressLine.matched': {
		kind: 'string',
		title: 'string'
	} satisfies Spec<ProgressOf<'matched'>>,
	'ProgressLine.links_fetched': {
		kind: 'string',
		provider: 'string'
	} satisfies Spec<ProgressOf<'links_fetched'>>,
	DownloadProgress: { line: 'string' } satisfies Spec<DownloadProgress>,
	DownloadResponse: { dest_dir: 'string' } satisfies Spec<DownloadResponse>,
	AvailabilityResponse: {
		available: 'boolean',
		episode_count: 'number|null',
		extra_episodes: 'string[]',
		episode_count_approximate: 'boolean',
		gate_refused: '?boolean',
		provider: 'string|null',
		unsettled: '?string',
		versions: '?object[]'
	} satisfies Spec<AvailabilityResponse>,
	AvailabilityBatchResponse: {
		cached: 'object',
		playable_episode_counts: 'object'
	} satisfies Spec<AvailabilityBatchResponse>,
	Ended: {
		ticket: 'number',
		landed: 'boolean'
	} satisfies Spec<RefillEnded>,
	TokensResponse: {
		access_token: 'string',
		refresh_token: 'string|null',
		expires_at_epoch_s: 'number'
	} satisfies Spec<Tokens>,
	UserProfile: {
		provider: 'string',
		user_id: 'string',
		username: 'string',
		avatar_url: 'string|null',
		stats: 'object|null'
	} satisfies Spec<UserProfile>,
	UserStats: {
		anime_count: 'number',
		mean_score_0_to_10: 'number|null'
	} satisfies Spec<UserStats>,
	ListEntry: {
		provider: 'string',
		media_id: 'number',
		mal_id: 'number|null',
		status: 'string',
		progress_episodes: 'number',
		score_0_to_100: 'number|null',
		updated_at_epoch_s: 'number',
		title: 'string'
	} satisfies Spec<ListEntry>,
	EntryView: {
		status: 'string',
		progress: 'number'
	} satisfies Spec<EntryView>,
	AuthUrlResponse: { url: 'string' } satisfies Spec<AuthUrlResponse>,
	ReleaseInfo: {
		tag: 'string',
		name: 'string',
		url: 'string',
		publishedAt: 'string',
		body: 'string'
	} satisfies Spec<ReleaseInfo>
};

/** Each body the backend reads, by its backend name: the spec of the
 *  renderer type that sends it, and the fields the backend reads that
 *  the type deliberately never carries. */
const REQUESTS: Record<string, { spec: Record<string, string>; unsent?: string[] }> = {
	PlayArgs: {
		spec: {
			title: 'required',
			episode: 'required',
			mode: 'required',
			episode_count: 'optional',
			year: 'optional',
			alt_titles: 'optional',
			prefetch: 'optional',
			kitsu_id: 'optional',
			subtype: 'optional',
			version: 'optional',
			guess_row: 'optional',
			guess_id: 'optional',
			caption: 'optional'
		} satisfies BodySpec<PlayArgs>,
		// Added to every play by `withUiLocale` in api.ts, not by the type.
		unsent: ['locale']
	},
	DownloadArgs: {
		spec: {
			title: 'required',
			episode: 'required',
			mode: 'required',
			quality: 'optional',
			episode_count: 'optional',
			year: 'optional',
			subtype: 'optional',
			alt_titles: 'optional',
			kitsu_id: 'optional',
			download_dir: 'optional',
			version: 'optional',
			caption: 'optional'
		} satisfies BodySpec<DownloadArgs>,
		// Added to the stream's query by `downloadStream`.
		unsent: ['locale']
	},
	AvailabilityArgs: {
		spec: {
			title: 'required',
			mode: 'required',
			alt_titles: 'optional',
			episode_count: 'optional',
			year: 'optional',
			kitsu_id: 'optional',
			subtype: 'optional',
			status: 'optional',
			background: 'optional',
			bypass_cache: 'optional'
		} satisfies BodySpec<AvailabilityArgs>
	},
	AvailabilityBatchArgs: {
		spec: { kitsu_ids: 'required', mode: 'required' } satisfies BodySpec<AvailabilityBatchBody>
	},
	AvailabilityWarmArgs: {
		spec: { items: 'required' } satisfies BodySpec<AvailabilityWarmBody>
	},
	LaunchArgs: {
		spec: {
			stream_url: 'required',
			referer: 'optional',
			title: 'optional',
			player_command: 'required',
			player_kind: 'optional',
			custom_args_template: 'optional'
		} satisfies BodySpec<LaunchExternalPlayerArgs>,
		// The renderer launches external players through the play
		// routes, which attach a resolve's tracks themselves.
		unsent: ['subtitle_urls', 'subtitles_off']
	},
	CreateSessionArgs: {
		spec: { upstream_url: 'required', referer: 'required' } satisfies BodySpec<CreateSessionArgs>,
		// The route refuses any: the resolver attaches a session's tracks.
		unsent: ['subtitles']
	},
	Config: {
		spec: {
			locale: 'required',
			mode: 'required',
			download_quality: 'required',
			external_player: 'required',
			external_player_kind: 'required',
			external_player_custom_args: 'required',
			syncplay_binary: 'required',
			image_cache_cap_mb: 'required',
			auto_play_next: 'required',
			cache_resolutions: 'required',
			download_bottom_bar_enabled: 'required',
			auto_skip_op: 'required',
			auto_skip_ed: 'required',
			use_custom_player_controls: 'required',
			update_include_prereleases: 'required',
			primary_account: 'required'
		} satisfies BodySpec<Config>
	},
	RuledOutRequest: {
		spec: {
			key: 'required',
			title: 'required',
			episode: 'required',
			hits: 'required',
			search: 'required'
		} satisfies BodySpec<GuessRuledOutBody>
	},
	TitleMatchBody: {
		spec: {
			title: 'required',
			cour: 'required',
			kitsu_id: 'required',
			provider: 'required',
			year: 'optional'
		} satisfies BodySpec<TitleMatchBody>
	},
	SearchBody: { spec: { query: 'required' } satisfies BodySpec<KitsuSearchBody> },
	KitsuByMalIdsBody: { spec: { mal_ids: 'required' } satisfies BodySpec<KitsuByMalIdsBody> },
	WaitBody: { spec: { tickets: 'required' } satisfies BodySpec<RefillsBody> },
	AuthUrlRequest: {
		spec: { state: 'required', pkce: 'required' } satisfies BodySpec<AuthUrlRequest>
	},
	ExchangeCodeRequest: {
		spec: { code: 'required', pkce: 'required' } satisfies BodySpec<ExchangeCodeRequest>
	},
	PkceWire: {
		spec: {
			verifier: 'required',
			challenge: 'required',
			method: 'required'
		} satisfies BodySpec<PkceWire>
	},
	RefreshRequest: { spec: { refresh_token: 'required' } satisfies BodySpec<RefreshBody> },
	UpdateProgressRequest: {
		spec: {
			kitsu_id: 'required',
			progress: 'required',
			status: 'optional'
		} satisfies BodySpec<UpdateProgressBody>,
		// The editor's score is not written from the renderer.
		unsent: ['score']
	},
	SetEntryRequest: {
		spec: {
			kitsu_id: 'required',
			status: 'optional',
			progress: 'optional'
		} satisfies BodySpec<SetEntryBody>
	}
};

// — The enums, each list proven to be exactly its renderer type ————

const UNSETTLED = ['refused', 'unanswered', 'past_affinity', 'superseded'] as const;
same<(typeof UNSETTLED)[number], UnsettledReason>(true);
const STREAM_PROVIDERS = ['hianime'] as const;
same<(typeof STREAM_PROVIDERS)[number], StreamProvider>(true);
const PLAYER_KINDS = ['mpv', 'vlc', 'iina', 'custom'] as const;
same<(typeof PLAYER_KINDS)[number], ExternalPlayerKind>(true);
const SEARCH_ORDERS = ['relevance', 'year', 'rating'] as const;
same<(typeof SEARCH_ORDERS)[number], SearchOrder>(true);
const LIST_STATUSES = [
	'planning',
	'watching',
	'completed',
	'paused',
	'dropped',
	'rewatching'
] as const;
same<(typeof LIST_STATUSES)[number], ListStatus>(true);
same<EntryView['status'], ListStatus>(true);
same<ListEntry['status'], ListStatus>(true);
const PROVIDERS = ['anilist', 'mal', 'inhouse'] as const;
same<(typeof PROVIDERS)[number], Provider>(true);
same<UserProfile['provider'], Provider>(true);
same<ListEntry['provider'], Provider>(true);
const PKCE_METHODS = ['plain', 'S256'] as const;
same<(typeof PKCE_METHODS)[number], PkceWire['method']>(true);
same<(typeof MEDIA_KINDS)[number], CreateSessionResponse['media_kind']>(true);
same<(typeof VERSION_KINDS)[number], VersionInfo['kind']>(true);
same<ExternalPlayerKind, Config['external_player_kind']>(true);
same<SidecarTrack, SessionSubtitle>(true);
const PROGRESS_KINDS = ['searching', 'matched', 'links_fetched'] as const;
same<(typeof PROGRESS_KINDS)[number], PlayProgress['kind']>(true);

const ENUMS: Record<string, readonly string[]> = {
	MediaKind: MEDIA_KINDS,
	VersionKind: VERSION_KINDS,
	UnsettledReason: UNSETTLED,
	ProviderId: STREAM_PROVIDERS,
	ExternalPlayerKind: PLAYER_KINDS,
	SearchOrder: SEARCH_ORDERS,
	ListStatus: LIST_STATUSES,
	'EntryView.status': LIST_STATUSES,
	ProviderKind: PROVIDERS,
	PkceMethod: PKCE_METHODS
};

// — The error envelope ——————————————————————————————————————————————

const ERROR_SPEC = {
	kind: 'string',
	key: '?string',
	detail: '?string',
	status: '?number',
	retry_after_secs: '?number|null',
	refused_by: '?string',
	binary: '?string',
	stage: '?string',
	host: '?string|null'
} satisfies Spec<AniErrorPayload>;

/** The error kinds the renderer compares an error's `kind` against. */
const ERROR_KINDS_NAMED = [
	'config',
	'episode_unavailable',
	'ffmpeg_missing',
	'gate_refused',
	'io',
	'network',
	'no_results',
	'parse_failed',
	'player_spawn_failed',
	'rate_limited',
	'scraper',
	'stream_server_failed',
	'syncplay_spawn_failed',
	'timeout',
	'upstream'
];

// — The tests ———————————————————————————————————————————————————————

describe('the renderer holds the backend wire contract', () => {
	it('names a renderer type for every response, body and enum the backend publishes', () => {
		expect(sorted(Object.keys(RESPONSES))).toEqual(sorted(Object.keys(CONTRACT.responses)));
		expect(sorted(Object.keys(REQUESTS))).toEqual(sorted(Object.keys(CONTRACT.requests)));
		expect(sorted(Object.keys(ENUMS))).toEqual(sorted(Object.keys(CONTRACT.enums)));
	});

	it.each(Object.keys(CONTRACT.responses))('response %s', (name) => {
		const { full, min } = CONTRACT.responses[name];
		expect(responseDrift(name, [full, min], RESPONSES[name] ?? {})).toEqual([]);
	});

	it.each(Object.keys(CONTRACT.requests))('request body %s', (name) => {
		const entry = REQUESTS[name] ?? { spec: {} };
		expect(requestDrift(name, CONTRACT.requests[name], entry.spec, entry.unsent)).toEqual([]);
	});

	it.each(Object.keys(CONTRACT.enums))('enum %s', (name) => {
		expect(sorted(ENUMS[name] ?? [])).toEqual(sorted(CONTRACT.enums[name]));
	});

	it('the progress kinds are the backend progress lines', () => {
		const lines = Object.keys(CONTRACT.responses)
			.filter((n) => n.startsWith('ProgressLine.'))
			.map((n) => n.slice('ProgressLine.'.length));
		expect(sorted(PROGRESS_KINDS)).toEqual(sorted(lines));
	});

	it('the error envelope declares every field an error carries', () => {
		// The backend always sends `key`; the type leaves it optional for
		// the `{ kind, status }` api.ts builds from a body that is not JSON.
		expect(responseDrift('AniErrorPayload', CONTRACT.errors, ERROR_SPEC, ['key'])).toEqual([]);
	});

	it('every error kind the renderer names is one the backend sends', () => {
		const sent = new Set(CONTRACT.errors.map((e) => e.kind));
		expect(ERROR_KINDS_NAMED.filter((k) => !sent.has(k))).toEqual([]);
	});

	it('the Watch Later bridge cap is the backend cap', () => {
		expect(WATCH_LATER_BRIDGE_MAX_IDS).toBe(CONTRACT.constants.WATCH_LATER_BRIDGE_MAX_IDS);
	});

	it.each(MEDIA_KINDS)('a %s session plays at the URL the backend gives it', (kind) => {
		const { base, session, urls } = CONTRACT.media_urls;
		expect(buildMediaUrl(base, session, kind)).toBe(urls[kind]);
	});
});

// — The checks catch what they are for ——————————————————————————————

describe('responseDrift', () => {
	const full = { a: 'x', b: null, c: [1], d: true };
	const min = { a: 'x', b: null, c: [] };

	it('passes a spec that says what the samples show', () => {
		expect(
			responseDrift('T', [full, min], { a: 'string', b: 'null', c: 'number[]', d: '?boolean' })
		).toEqual([]);
	});

	it('names a field the backend sends and the type lacks', () => {
		expect(responseDrift('T', [full, min], { a: 'string', b: 'null', c: 'number[]' })).toEqual([
			'T.d: sent, but the type does not declare it'
		]);
	});

	it('names a field the type declares and the backend never sends', () => {
		expect(
			responseDrift('T', [full, min], {
				a: 'string',
				b: 'null',
				c: 'number[]',
				d: '?boolean',
				e: '?string'
			})
		).toEqual(['T.e: declared, but the backend never sends it']);
	});

	it('names optionality that disagrees, either way', () => {
		expect(
			responseDrift('T', [full, min], { a: '?string', b: 'null', c: 'number[]', d: 'boolean' })
		).toEqual([
			'T.a: always sent, but the type marks it optional',
			'T.d: left out of some answers, but required'
		]);
	});

	it('names kinds that disagree, either way', () => {
		expect(
			responseDrift('T', [full, min], {
				a: 'string|null',
				b: 'null',
				c: 'string[]',
				d: '?boolean'
			})
		).toEqual([
			'T.a: the backend sends string, the type says null|string',
			'T.c: the backend sends number[], the type says string[]'
		]);
	});

	it('lets a named field stay optional though always sent', () => {
		expect(
			responseDrift('T', [full, min], { a: '?string', b: 'null', c: 'number[]', d: '?boolean' }, [
				'a'
			])
		).toEqual([]);
	});
});

describe('requestDrift', () => {
	const backend = { accepts: ['a', 'b', 'c'], requires: ['a'] };

	it('passes a body that sends what the backend reads', () => {
		expect(requestDrift('B', backend, { a: 'required', b: 'optional' }, ['c'])).toEqual([]);
	});

	it('names a field the backend does not read', () => {
		expect(
			requestDrift('B', backend, { a: 'required', b: 'optional', z: 'optional' }, ['c'])
		).toEqual(['B.z: sent, but the backend reads no such field']);
	});

	it('names a required field the type may leave out', () => {
		expect(requestDrift('B', backend, { a: 'optional', b: 'optional' }, ['c'])).toEqual([
			'B.a: the backend requires it, the type may leave it out'
		]);
	});

	it('names a field the backend reads that the type neither sends nor lists', () => {
		expect(requestDrift('B', backend, { a: 'required', b: 'optional' })).toEqual([
			'B.c: the backend reads it, the type does not declare it'
		]);
	});

	it('names a stale unsent entry', () => {
		expect(requestDrift('B', backend, { a: 'required', b: 'optional' }, ['c', 'y'])).toEqual([
			'B.y: listed as unsent, but the backend reads no such field'
		]);
	});
});
