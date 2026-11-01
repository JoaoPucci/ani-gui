// Which look a Continue Watching card is in.
//
// Stub with the behaviour the home page has today: a card draws
// whatever is known as it arrives — the placeholder while it is
// matched, the poster once it is, the episode thumbnail once the
// episode details land — and nothing is pinned or remembered.

import type { KitsuAnimeRef, KitsuEpisode } from '$lib/api';

/** How long a matched card waits for its episode details. Today: not at all. */
export const EPISODE_WAIT_CAP_MS = 0;

export interface CardImage {
	src: string;
	fallback: string | null;
}

export interface SettledLook {
	image: CardImage | null;
	matchId: string | null;
}

export interface CardText {
	tag: number | null;
	title: string | null;
	episodeLabel: number | null;
}

export interface LookInputs {
	match: KitsuAnimeRef | null | undefined;
	episode: KitsuEpisode | null | undefined;
	capReached: boolean;
	text: CardText | null;
}

export type ProxyUrl = (url: string | null) => string | null;

export function posterSource(match: KitsuAnimeRef | null | undefined): string | null {
	const p = match?.poster_image;
	return p?.medium ?? p?.large ?? p?.small ?? p?.original ?? null;
}

export function cardText(
	episode: KitsuEpisode | null | undefined,
	singleVideo: boolean,
	episodeNumber: number
): CardText {
	const title = episode?.canonical_title ?? null;
	return {
		tag: singleVideo ? null : episodeNumber,
		title,
		episodeLabel: title === null && !singleVideo ? episodeNumber : null
	};
}

export function decideLook(inputs: LookInputs, proxy: ProxyUrl): SettledLook | null {
	const src =
		proxy(inputs.episode?.thumbnail?.original ?? null) ?? proxy(posterSource(inputs.match));
	return { image: src ? { src, fallback: null } : null, matchId: inputs.match?.id ?? null };
}

export interface RememberedCard {
	look: SettledLook;
	text: CardText | null;
}

export interface CardLookState {
	look: SettledLook | null;
	text: CardText | null;
	instant: boolean;
}

export const sessionLooks = new Map<string, RememberedCard>();

export function lookKey(entry: { id: string; ep_no: string }): string {
	return `${entry.id}|${entry.ep_no}`;
}

export function createLookBook(proxy: ProxyUrl, remembered: Map<string, RememberedCard>) {
	void remembered;
	return {
		look(_entryId: string, _key: string, inputs: LookInputs): CardLookState {
			return { look: decideLook(inputs, proxy), text: inputs.text, instant: false };
		}
	};
}

export type LookBook = ReturnType<typeof createLookBook>;

export function visibleImage(
	image: CardImage | null,
	failed: Readonly<Record<string, true>>
): string | null {
	void failed;
	return image?.src ?? null;
}

export interface CapTimerDeps {
	capMs: number;
	schedule: (fn: () => void, ms: number) => unknown;
	cancel: (handle: unknown) => void;
	onElapsed: (entryId: string) => void;
}

export function createCapTimers(deps: CapTimerDeps) {
	void deps;
	return {
		arm(entryId: string) {
			void entryId;
		},
		dispose() {}
	};
}
