// Which look a Continue Watching card is in.
//
// A card's artwork depends on two answers that land at different
// times: the row's Kitsu match (the show's poster) and then the
// episode details (the episode's thumbnail). Drawing each answer as it
// arrived made a card go placeholder → poster → thumbnail. Here a card
// stays a skeleton until its final artwork is known and then keeps it:
//
//   - match pending                → skeleton
//   - no match                     → the two-letter placeholder
//   - episode details settled      → thumbnail, else poster, else placeholder
//   - episode details still out at the cap → poster, else placeholder
//
// The first look a card settles on is pinned for as long as the card
// offers the same show and episode, so a thumbnail that lands after the
// cap does not swap it. The look the card settles on without the cap
// (its episode details, or no match) is remembered for the session, so
// the next visit to Home draws the card in its final look at once.

import type { KitsuAnimeRef, KitsuEpisode } from '$lib/api';

/**
 * How long a matched card waits for its episode details before it
 * settles on the show's poster. Measured on a seeded profile of fifty
 * Continue rows, from a card's match to its episode details: 95% of
 * cards land within about 390 ms on a fresh launch and within about
 * 370 ms on a return to Home, so a card waits no longer than that
 * while a slow lookup (an episode page Kitsu has to be asked for)
 * settles on its poster instead of holding the skeleton.
 */
export const EPISODE_WAIT_CAP_MS = 400;

/** The artwork a card shows, and what to show if that one fails. */
export interface CardImage {
	src: string;
	fallback: string | null;
}

/** A card's settled artwork. `image: null` is the two-letter placeholder. */
export interface SettledLook {
	image: CardImage | null;
	/** The match the look was decided against; null when the row has none. */
	matchId: string | null;
	/** The episode the card offered when the look was decided; null unmatched. */
	episode: number | null;
}

/** The text a card carries beside its artwork. */
export interface CardText {
	/** The show's name as the card gives it. */
	show: string;
	/** The number on the artwork; null for a single-video show. */
	tag: number | null;
	/** The episode's own title, when Kitsu has one. */
	title: string | null;
	/** The "Episode N" line shown when there is no title; null when none. */
	episodeLabel: number | null;
}

export interface LookInputs {
	/** undefined while the row is being matched; null when it failed. */
	match: KitsuAnimeRef | null | undefined;
	/** undefined until the episode lookup settles. Details for an
	 *  episode other than `offered` count as not settled yet. */
	episode: KitsuEpisode | null | undefined;
	/** The episode the card offers (what its click plays); null unmatched. */
	offered: number | null;
	/** The episode the details were fetched for. Stub: not read yet. */
	episodeFor?: number;
	/** Whether the episode wait has run out for this card. */
	capReached: boolean;
	/** The card's text from live data; null while the row is unmatched. */
	text: CardText | null;
}

/** Turns a remote image URL into the one the card loads, or null. */
export type ProxyUrl = (url: string | null) => string | null;

/** The poster size a card uses, largest-fitting first. */
export function posterSource(match: KitsuAnimeRef | null | undefined): string | null {
	const p = match?.poster_image;
	return p?.medium ?? p?.large ?? p?.small ?? p?.original ?? null;
}

/**
 * The episode details as far as they describe the episode the card
 * offers. The row's details are fetched for one episode at a time and
 * re-fetched when the offered episode moves (a refined cap, a row read
 * again), so the page can briefly hold the previous episode's: those
 * are not this card's, and count as not arrived. A null answer (no
 * details found) stands, as does anything when no episode is offered.
 */
export function offeredEpisode(
	episode: KitsuEpisode | null | undefined,
	offered: number | null,
	episodeFor?: number
): KitsuEpisode | null | undefined {
	void episodeFor;
	if (!episode || offered === null) return episode;
	return episode.number === offered || episode.relative_number === offered ? episode : undefined;
}

/** The text a card shows: the show's name and the episode line. */
export function cardText(
	episode: KitsuEpisode | null | undefined,
	singleVideo: boolean,
	episodeNumber: number,
	show: string
): CardText {
	const title = episode?.canonical_title ?? null;
	return {
		show,
		tag: singleVideo ? null : episodeNumber,
		title,
		episodeLabel: title === null && !singleVideo ? episodeNumber : null
	};
}

function posterLook(match: KitsuAnimeRef, offered: number | null, proxy: ProxyUrl): SettledLook {
	const poster = proxy(posterSource(match));
	return {
		image: poster ? { src: poster, fallback: null } : null,
		matchId: match.id,
		episode: offered
	};
}

/**
 * The look a card settles on given what is known, or null while it is
 * still a skeleton. Pure: pinning the first answer is the book's job.
 */
export function decideLook(inputs: LookInputs, proxy: ProxyUrl): SettledLook | null {
	const { match, offered } = inputs;
	if (match === undefined) return null;
	if (match === null) return { image: null, matchId: null, episode: null };
	const episode = offeredEpisode(inputs.episode, offered);
	if (episode !== undefined) {
		const thumb = proxy(episode?.thumbnail?.original ?? null);
		const poster = posterLook(match, offered, proxy);
		if (!thumb) return poster;
		return {
			image: { src: thumb, fallback: poster.image?.src ?? null },
			matchId: match.id,
			episode: offered
		};
	}
	return inputs.capReached ? posterLook(match, offered, proxy) : null;
}

/**
 * The image a settled look shows once failures are counted: its own,
 * else its fallback, else none (the placeholder).
 */
export function visibleImage(
	image: CardImage | null,
	failed: Readonly<Record<string, true>>
): string | null {
	if (!image) return null;
	if (!failed[image.src]) return image.src;
	if (image.fallback && !failed[image.fallback]) return image.fallback;
	return null;
}
