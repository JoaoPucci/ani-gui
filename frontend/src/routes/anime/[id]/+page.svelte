<!--
  Anime detail v2 — editorial spread, but no longer synopsis-dominant.
  Composition:
    - Hero band (cover_image with blurred-poster fallback) + parallax.
    - Masthead: poster hangs into the hero. Right column carries title +
      action row (Play / Download / External) + Sub-Dub + Quality + meta-row.
    - 2-column body: left = synopsis (≤60ch, drop cap kept), right =
      Episodes panel (12 placeholder tiles with per-anime accent on hover).
      Vertical accent-tinted hairline divides the columns.
  CLI features represented even when wiring is M2:
    Mode (sub/dub) and Quality persist via settingsGet/settingsPut today.
    Play / Download / External are wired to TODOs with inline notice.
-->
<script lang="ts">
	import {
		failedHeroImages,
		naturalRatio,
		noteHeroImageFailed,
		pickHeroImage,
		type HeroOrigin
	} from '$lib/hero/framing';
	import { browserStorage, loadHeroPrefs, saveHeroPrefs, type HeroPrefs } from '$lib/hero/prefs';
	import HeroFramingSwitch from '$lib/hero/HeroFramingSwitch.svelte';
	import { onDestroy, onMount, untrack } from 'svelte';
	import { page } from '$app/state';
	import { beforeNavigate, goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import {
		altTitlesFromKitsu,
		yearFromKitsuRef,
		checkAvailability,
		historyByKitsu,
		historyNumber,
		imageProxyUrl,
		kitsuAnimeDetail,
		kitsuEpisodes,
		kitsuSearch,
		markWatched,
		playStream,
		airingGet,
		settingsGet,
		settingsPut,
		type AiringStatus,
		type Config,
		type HistoryEntry,
		type KitsuAnimeRef,
		type KitsuEpisode,
		type VersionInfo,
		type VersionKind
	} from '$lib/api';
	import { ctaState } from '$lib/detail/cta-state';
	import {
		asksOtherAudio,
		unavailableMessage,
		watchOtherAudio
	} from '$lib/detail/unavailable-copy';
	import type { OtherAudio } from '$lib/play/error-copy';
	import { subtypeLabel } from '$lib/search/subtype-label';
	import { describeSettingsFailure } from '$lib/settings/failure-copy';
	import { describeError } from '$lib/play/error-copy';
	import { describePlayFailureFromCache } from '$lib/play/other-audio';
	import {
		claimPlay,
		playArgsFor,
		playMode,
		playQuality,
		recordPlayed,
		runOwnedPlay
	} from '$lib/play/owned-play';
	import { progressLabel } from '$lib/play/format';
	import { airingPending, epAirState, formatAirDate } from '$lib/detail/episode-airing';
	import { datedAired, withAiredFloor } from '$lib/detail/aired-evidence';
	import {
		capGateAnswerFrom,
		createCapGateProbe,
		type CapGateRefresh
	} from '$lib/detail/cap-gate-probe';
	import {
		createAvailabilityWriteback,
		type AvailabilityPatch
	} from '$lib/detail/availability-writeback';
	import { startAvailabilityLookup } from '$lib/detail/availability-lookup';
	import { lookupResume } from '$lib/detail/resume-lookup';
	import { toastStore } from '$lib/toasts/store.svelte';
	import { runUnairedClick } from '$lib/detail/unaired-click';
	import { airedCap, beyondPlayable, displayCap, minCap } from '$lib/detail/episode-caps';
	import { getLocale } from '$lib/paraglide/runtime';
	import { filterAvailable } from '$lib/availability/filter';
	import { settle, settleOut } from '$lib/transitions/settle';
	import ErrorOverlay from '$lib/components/ErrorOverlay.svelte';
	import LoadingOverlay from '$lib/components/LoadingOverlay.svelte';
	import PosterCard from '$lib/components/PosterCard.svelte';
	import Strip from '$lib/components/Strip.svelte';
	import { accentFor } from '$lib/design/accent';
	import {
		clearBackgroundForOtherVersions,
		clearBackgroundForShow,
		clearForShow,
		getOrFire,
		makeKey
	} from '$lib/play/play-cache';
	import { createShowReleases } from '$lib/play/show-release';
	import { createNavigationClaims } from '$lib/navigation/claims';
	import { detailWarmTargets } from '$lib/play/warm-plan';
	import { buildPlayQuery } from '$lib/play/play-url';
	import {
		applyChoice,
		selectedVersion,
		storedChoice,
		versionsToOffer,
		versionsInMode
	} from '$lib/versions/version-choice';
	import { handleRadioKey } from '$lib/versions/radio-keys';
	import { versionLabel } from '$lib/versions/version-label';
	import { followPlayed } from '$lib/play/played-episode';
	import { computePlayLabel, isSingleVideo } from '$lib/detail/play-label';
	import { isMusicSubtype } from '$lib/history/resolve';
	import { audioKey, mergeRow, rowFor, type AvailabilityRow } from '$lib/availability/row';
	import { pickResumeEpisode } from '$lib/play/next-episode';
	import { extraOnPage, parseHistoryEpisode, wholeEpisodeCount } from '$lib/play/episode-number';
	import { fallbackTileTitle } from '$lib/play/tile-title';
	import { markStarted, movePosition, readPosition } from '$lib/play/watch-position';
	import { joinResumeRefresh, refreshedResume } from '$lib/detail/resume-refresh';
	import { warmResumeEpisode } from '$lib/detail/resume-warm';
	import { settledWithin } from '$lib/detail/resume-join';
	import { syncWatchedToTrackers } from '$lib/account/push-watched';
	import { accountStore } from '$lib/account/store.svelte';
	import { getEntry } from '$lib/account/entry-api';
	import { readEntryWithRetry, readFailureNotice } from '$lib/account/list-entry-read';
	import { freshBearerFor } from '$lib/account/fresh-bearer';
	import ListEntryEditor from '$lib/components/ListEntryEditor.svelte';
	import type { EntryView } from '$lib/account/types';
	import { createEpisodePageCache, resetEpisodePageCache } from '$lib/detail/episode-page-cache';
	import { downloadDefaultDir as downloadDefaultDirApi } from '$lib/api';
	import DownloadConfirm from '$lib/components/DownloadConfirm.svelte';
	import type { DownloadArgs } from '$lib/api';
	import { buildDownloadArgs } from '$lib/download/build-args';
	import { decideEpisodeFetchAction, parsePageParam } from '$lib/history/url-deeplink';
	import { breadcrumb } from '$lib/breadcrumb';
	import { m } from '$lib/paraglide/messages';

	let detail = $state<KitsuAnimeRef | null>(null);
	let config = $state<Config | null>(null);
	const id = $derived(page.params.id ?? '');
	const accent = $derived(id ? accentFor(id) : 'var(--accent-ink)');
	let error = $state<{ headline: string; detail: string | null } | null>(null);
	let scrollY = $state(0);

	// True when Kitsu marks this show as a one-video work — used by
	// the CTA derivation (`playLabel`), the play-failure error
	// headline, AND the structural hide of the Episodes section
	// below (a movie's "Episodes" strip would show a single tile,
	// which is pure noise). See lib/detail/play-label.ts.
	const singleVideo = $derived(
		isSingleVideo(detail?.subtype ?? null, detail?.episode_count ?? null, detail?.status ?? null)
	);

	// Availability probe — runs after detail loads, and again when the
	// audio changes. null = no answer yet for this show in this audio /
	// network error (lazy fallback handles the click);
	// true = the provider has at least one candidate; false = the show
	// isn't in the provider's catalog (Kitsu indexes Western animation
	// like "Arcane Season 2" too — the play CTA there is a dead end).
	//
	// It is one field of the provider's row for the show
	// ($lib/availability/row), which every accepted answer writes
	// and which is read for the show and audio on screen: an answer
	// about one audio says nothing about the other. A music title is
	// unavailable in both by this page's own rule, asked or not.
	let availabilityRow = $state<AvailabilityRow | null>(null);
	const rowKey = $derived(audioKey(detail?.id, playMode(config)));
	const knownRow = $derived(rowFor(availabilityRow, rowKey));
	const availability = $derived(isMusicSubtype(detail?.subtype ?? null) ? false : knownRow.listed);
	// True once the availability probe settled (result, error, or the
	// music-subtype skip). The prefetch warm waits on it: firing while
	// playableEpisodeCount is still null would treat the cap as
	// unbounded and resolve aired-but-uncatalogued padded tiles.
	let availabilityResolved = $state(false);
	// The current lookup's own verdict, for the warm: null while the
	// question is open or after it failed, never one retained from
	// an earlier lookup — `availability` above keeps an earlier answer
	// in the same audio on failure, for the call to action, and the warm
	// acts only on what this lookup said.
	let warmListed = $state<boolean | null>(null);

	// The provider's availableEpisodes for the chosen candidate, populated
	// alongside availability. This is the authoritative "what's
	// streamable RIGHT NOW" number — Kitsu's episode_count lags real
	// airing for ongoing shows (One Piece: Kitsu 1106, the provider 1161).
	const playableEpisodeCount = $derived(knownRow.count);

	// The show's versions, when the provider lists it twice ("Versions
	// of one pick" in docs/title-resolution.md), as the availability
	// answer names them; and the version remembered for the show,
	// which every play and download request names. Nothing remembered
	// leaves the backend's default, the uncensored copy.
	let showVersions = $state<VersionInfo[]>([]);
	let versionChoice = $state<VersionKind | undefined>(undefined);
	// The mode the versions were answered in: what they say of each
	// version's mode is that mode's.
	let versionsMode = $state<'sub' | 'dub'>('sub');
	const offeredVersions = $derived.by(() =>
		versionsToOffer(versionsInMode(showVersions, versionsMode, availabilityMode))
	);
	const shownVersion = $derived(selectedVersion(offeredVersions, versionChoice));
	function chooseVersion(kind: VersionKind) {
		// The version already shown is the one requests play; a click on
		// it changes nothing, and must not re-warm under another key.
		if (kind === shownVersion) return;
		versionChoice = applyChoice(id, kind);
		// The strip re-warms under the chosen version's keys; the
		// version left's warms are for a listing nobody will play.
		if (id) clearBackgroundForOtherVersions(id, versionChoice);
	}
	const onVersionKey = (e: KeyboardEvent, index: number) =>
		handleRadioKey(e, index, offeredVersions.length, (i) => chooseVersion(offeredVersions[i].kind));
	/**
	 * Clicking a cap-gated tile re-asks the provider rather than doing
	 * nothing. The playable count is a snapshot — 24h for an ongoing
	 * show — and the catalogue catches up inside that window, so the
	 * tile can be dead for an episode that became streamable hours
	 * ago.
	 *
	 * It BLOCKS, like every other long action here. `actionBusy`
	 * raises the same overlay a play click does, so there is no window
	 * in which a second tile can be clicked. The route can still change
	 * under it — Back, or a link reached from the keyboard, which the
	 * overlay does not hold — and the context the answer is checked
	 * against says so: another show, a page left, or a navigation
	 * started that leaves it (`navigationClaims`). A cleared count
	 * falls straight through into startPlay, which holds the overlay
	 * up — the caption just changes from checking to the usual play
	 * progress.
	 */
	/**
	 * The audio mode the re-ask asks about. The provider catalogues sub and
	 * dub separately and dub lags, so this is part of the question
	 * rather than a detail of it: the probe sends it, and the context
	 * the answer is checked against is keyed on it. Settings land after
	 * the page does, so a click in that gap asks with the fallback and
	 * the real mode can arrive while the answer is out.
	 */
	const capGateMode = (): 'sub' | 'dub' => playMode(config);
	/**
	 * The context once this page is gone.
	 *
	 * Leaving the route entirely — Back, or any history navigation —
	 * destroys the component but not the promise. The show and the
	 * mode it closed over are unchanged, so a guard that compares only
	 * those sees a perfectly current answer and acts on it: `startPlay`
	 * runs, and its `goto` hauls the user back into playback on a page
	 * they left. Nothing about the question changed; there is simply
	 * nobody left asking.
	 *
	 * Cannot collide with a real context, which always carries `:`
	 * separators between its parts.
	 */
	/**
	 * Which visit to this route the answer belongs to.
	 *
	 * The show and the mode describe the ROW being asked about, and
	 * both come back when the user does. SvelteKit reuses this
	 * component across an A→B→A round trip, so nothing is destroyed
	 * and a click abandoned two navigations ago finds the context it
	 * left behind and acts on it. This advances on every id
	 * transition, so a question can only be answered to the page that
	 * asked it.
	 *
	 * Derived from the route param rather than the loaded detail: the
	 * detail is briefly null on any load, and treating that as a
	 * departure would invalidate a probe nobody navigated away from.
	 *
	 * A derived re-evaluates when read, so an A→B→A trip advances it
	 * only because effects on this page read `id` while it was B; the
	 * awaited flows that compare a captured visit after their await
	 * rely on that.
	 */
	const visit = $derived.by(() => {
		void id;
		return (visitCounter += 1);
	});
	let visitCounter = 0;
	const GONE = 'gone';
	let gone = false;
	onDestroy(() => {
		gone = true;
	});
	/**
	 * A navigation the viewer started. The page stays on screen until
	 * it lands, with the same show and visit, and an action answering
	 * in that window that went on to play would cancel the navigation
	 * they asked for with its own `goto`. Actions that end in a play
	 * claim navigation as they start, and their re-checks carry the
	 * epoch in their context ($lib/navigation/claims).
	 *
	 * Only a navigation that leaves this page counts: another path,
	 * which includes another show, or out of the app. One that changes
	 * only the query — the episode pager, which the overlay covers from
	 * the pointer but not from the keyboard, or a link to the URL on
	 * screen — keeps the show on screen, and the action stays the
	 * viewer's.
	 */
	const navigationClaims = createNavigationClaims();
	beforeNavigate((nav) => navigationClaims.started(nav));
	/**
	 * Write back what the re-ask learned about the show.
	 *
	 * The lookup replaces the backend's whole cached row, so the page
	 * has to take the whole row too — not just the number the click
	 * was about. A delisted show would otherwise keep every tile
	 * enabled, a shrunken cap would leave tiles enabled above it, and
	 * a special catalogued since page load would stay invisible.
	 *
	 * Nulls mean "the answer did not establish this", so they are
	 * skipped rather than written.
	 */
	/**
	 * Who owns which part of the row while a page-load lookup and a
	 * re-ask are both out. The re-ask wins where it answered — it is
	 * newer and it skipped the cache — but only where it answered:
	 * an unconfirmed count leaves that field open to the lookup.
	 */
	const writeback = createAvailabilityWriteback(() =>
		gone ? GONE : `${visit}:${detail?.id ?? ''}:${capGateMode()}`
	);
	function applyAvailabilityPatch(patch: AvailabilityPatch) {
		// The writeback and the re-ask have both checked that the answer
		// is about the show and audio on screen, so it is stamped with
		// them.
		availabilityRow = mergeRow(availabilityRow, rowKey, patch);
		// A patch that carries the verdict is the current lookup's or a
		// re-ask's that won it — either way the warm's too.
		if (patch.available !== undefined) warmListed = patch.available;
		if (patch.versions !== undefined) {
			// A patch lands only in the context it was asked in — this
			// show, this audio — so the versions are this mode's.
			showVersions = patch.versions;
			versionsMode = capGateMode();
		}
	}
	function applyCapGateRefresh(refresh: CapGateRefresh) {
		applyAvailabilityPatch(writeback.refresh(refresh));
		// The re-probe paid for a listing: the resume row is read again.
		resumeRefresh.again();
	}
	const capGate = createCapGateProbe({
		probe: async (signal) => {
			const d = detail;
			if (!d) return null;
			const r = await checkAvailability(
				{
					title: d.canonical_title,
					mode: capGateMode(),
					alt_titles: altTitlesFromKitsu(d),
					episode_count: d.episode_count ?? undefined,
					year: yearFromKitsuRef(d) ?? undefined,
					kitsu_id: d.id,
					subtype: d.subtype ?? undefined,
					status: d.status ?? undefined,
					background: false,
					// Without this the lookup answers from the very row
					// being questioned and never reaches the provider.
					bypass_cache: true
				},
				{ signal }
			);
			// The whole row, not one field of it — an unsettled answer
			// as none, a refusal by name as the show unavailable. The
			// controller decides which parts of a fresh answer are safe
			// to write back.
			return capGateAnswerFrom(r);
		},
		currentContext: () =>
			gone ? GONE : `${visit}:${navigationClaims.epoch}:${detail?.id ?? ''}:${capGateMode()}`,
		onCleared: (episode, _count, refresh) => {
			applyCapGateRefresh(refresh);
			// Overlay stays up; startPlay owns it from here.
			void startPlay(episode);
		},
		onStillGated: (_episode, refresh) => {
			applyCapGateRefresh(refresh);
			actionBusy = false;
			actionProgress = null;
			toastStore.push({ kind: 'info', message: m.detail_ep_recheck_still_gated() });
		},
		onFailed: (_episode, refresh) => {
			// An unconfirmed answer still settled whether the show is
			// listed, even though it settled nothing about this episode.
			if (refresh) applyCapGateRefresh(refresh);
			actionBusy = false;
			actionProgress = null;
			toastStore.push({ kind: 'error', message: m.detail_ep_recheck_failed() });
		},
		// The user is on another show now. Let the page go, say
		// nothing — a message here would be about a title they left.
		onSuperseded: () => {
			actionBusy = false;
			actionProgress = null;
		}
	});

	// What a recheck is about, as a value: when it changes under an
	// in-flight recheck (another show on this route, another mode),
	// the recheck's answer would be discarded, so its probe is stopped
	// rather than left holding a limiter slot.
	// The visit, not the loaded detail, which is briefly null on any
	// load: a recheck nobody navigated away from must not be stopped.
	const capGateKey = $derived(`${visit}:${capGateMode()}`);
	$effect(() => {
		void capGateKey;
		return () => capGate.supersede();
	});

	/**
	 * Tile click for an episode the schedule calls unaired: refresh the
	 * schedule and the episode's page past their caches, then treat the
	 * episode as the fresh answer says ($lib/detail/unaired-click).
	 */
	async function onUnairedEpisode(n: number) {
		await runUnairedClick({
			show: id ?? null,
			episode: n,
			kitsuPageSize: KITSU_PAGE_SIZE,
			caption: m.detail_ep_unaired_recheck_busy(),
			isBusy: () => actionBusy,
			hold: (caption) => {
				actionBusy = true;
				actionProgress = caption;
			},
			heldCaption: () => actionProgress,
			release: () => {
				actionBusy = false;
				actionProgress = null;
			},
			fetch: (show, page) =>
				Promise.all([
					airingGet(show, { refresh: true }),
					kitsuEpisodes(show, page, { refresh: true })
				]),
			apply: ([schedule, eps], page) => {
				airingSchedule = schedule;
				kitsuPageCache.set(page, eps);
			},
			isAired: () => !epAirState(n, airing).unaired,
			beyondPlayable: () => beyondPlayable(n, playableEpisodeCount),
			currentContext: () => (gone ? GONE : `${visit}:${navigationClaims.epoch}:${id ?? ''}`),
			play: () => onPickEpisode(n),
			recheckProvider: () => onRecheckEpisode(n),
			notifyStillUnaired: () =>
				toastStore.push({ kind: 'info', message: m.detail_ep_unaired_still() }),
			notifyFailed: () => toastStore.push({ kind: 'error', message: m.detail_ep_recheck_failed() })
		});
	}

	/** Tile click for an episode the cap is currently hiding. */
	function onRecheckEpisode(n: number) {
		if (actionBusy) return;
		actionBusy = true;
		actionProgress = m.detail_ep_recheck_busy();
		capGate.request(n);
	}

	// Non-integer episode tags the provider has streamable (recap /
	// special episodes — e.g. ["1061.5"] for One Piece). Spliced
	// into the episode strip at their numeric position so the user
	// can navigate to them. Empty when the show has no extras.
	const extraEpisodes = $derived(knownRow.extraEpisodes);

	// The cap used for: resume button (defaultEpisode), Download All
	// total, Download range max, episode-list tile count. Prefers
	// the provider's count; falls back to Kitsu's announced total when
	// the provider isn't known yet (cold cache); null when neither is
	// known and the surface should treat the cap as unbounded.
	const episodeCap = $derived(playableEpisodeCount ?? detail?.episode_count ?? null);

	// Most-recent history entry (if any) whose provider show_id maps
	// to this Kitsu id, via the reverse cache stamped by every
	// successful play. When set, the primary CTA flips from "Play
	// episode 1" to "Continue · Episode N+1" — N taken from
	// resumeEntry.ep_no, capped at the show's known episode count so
	// we don't propose an episode that doesn't exist yet.
	let resumeEntry = $state<HistoryEntry | null>(null);

	// Episode list — fetched in parallel with the detail. Holds the
	// CURRENT page only (not concatenated) so a 500-episode show doesn't
	// produce a 500-tile vertical wall. Pagination replaces these.
	//
	// Two page sizes exist:
	//   UI_PAGE_SIZE  — visible tile count per UI page. ~3 rows × 4 cols
	//                   on widescreen, 4 × 3 on the narrower body width.
	//                   Smaller than the Kitsu cap so the row count feels
	//                   contained instead of running past the fold.
	//   KITSU_PAGE_SIZE — Kitsu's hard `page[limit]` cap. Backend fetches
	//                   come in multiples of this; the UI window slices
	//                   into them.
	// One UI page therefore maps to 1 or 2 Kitsu pages, kept in an
	// in-memory cache (kitsuPageCache) so prev/next is instant after
	// the first hop. Adjacent UI pages are prefetched after every load.
	// `rawWindowed` holds the unpadded Kitsu rows for the current
	// page; the rendered `episodes` view (declared after the airing
	// state it depends on) pads it reactively.
	let rawWindowed = $state<KitsuEpisode[] | null>(null);
	let episodesError = $state<string | null>(null);
	let episodesPage = $state(1);
	let episodesLoading = $state(false);
	let jumpInput = $state('');
	// Episode tile to scroll-to + briefly highlight after a Jump-to-ep
	// submit. Mirrors the prior Continue Watching deep-link behaviour.
	// Cleared on a 3.2s timeout once the tile is on screen so the
	// accent ring isn't permanent.
	let highlightEp = $state<number | null>(null);
	const UI_PAGE_SIZE = 12;
	const KITSU_PAGE_SIZE = 20;
	// Per-show — gets reset in the id-change effect below so a
	// /anime/A → /anime/B navigation doesn't serve A's cached page 1
	// thumbnails for the first paint of B. See lib/detail/episode-page-cache.
	const kitsuPageCache = createEpisodePageCache();

	// Airing schedule (AniList via the backend) for non-finished shows:
	// tiles past `airing.aired` render greyed instead of inviting a
	// doomed source resolution. null = unknown → every tile stays
	// interactive (epAirState never gates on unknown), so a failed
	// fetch degrades to today's behavior.
	let airingSchedule = $state<AiringStatus | null>(null);
	// The schedule can map to a different entry than this one (a
	// premiere split off as its own finished entry), so the episode
	// rows' own air dates are a floor under its aired count — every
	// gate below reads this, never the schedule alone.
	const airing = $derived(
		withAiredFloor(airingSchedule, datedAired(kitsuPageCache.values(), Date.now()))
	);
	// True once the airing question is answered for this show — fetched,
	// failed (stays unknown/ungated), or skipped for finished shows. The
	// prefetch warm waits on it so it can't race ahead of the schedule
	// and burn scraper slots on unaired episodes.
	let airingResolved = $state(false);
	$effect(() => {
		const currentId = id;
		const status = detail?.status;
		airingSchedule = null;
		airingResolved = false;
		if (!currentId || !status) return;
		if (status === 'finished') {
			airingResolved = true;
			return;
		}
		let cancelled = false;
		void airingGet(currentId)
			.then((a) => {
				if (!cancelled) airingSchedule = a;
			})
			.catch(() => {
				/* unknown airing data → tiles stay ungated */
			})
			.finally(() => {
				if (!cancelled) airingResolved = true;
			});
		return () => {
			cancelled = true;
		};
	});

	// In-flight beat between mount and the airing answer: tiles and
	// the primary actions stay inert so a quick click can't race the
	// schedule. Resolved-unknown stays ungated.
	const airingIsPending = $derived(airingPending(airingResolved, detail?.status));

	// CTA area render state. 'ready' includes the settled-but-unknown
	// verdict (probe errored, e.g. throttled provider searches) — the
	// old availability===null skeleton had no retry and pulsed forever
	// when the first probe hit a rate limit; the lazy click path
	// surfaces the real error instead.
	const cta = $derived(ctaState(availabilityResolved, availability));

	// What the cache says about the other audio while the notice that
	// replaces Play is up: it names the other audio when that has the
	// title ($lib/detail/unavailable-copy). Re-asked when the title or
	// the audio changes; a late answer to an old question is dropped.
	let noticeOtherAudio = $state<OtherAudio>(null);
	// Whether `availability`'s verdict is the provider refusing the
	// title by name: written with the verdict, by whoever owns it
	// ($lib/detail/availability-writeback).
	const availabilityRefused = $derived(knownRow.refused);
	// A music title is unavailable in both audios by this page's own
	// rule, so it never names the other one, whatever the cache holds.
	$effect(() => {
		const q = { kitsuId: id, mode: availabilityMode };
		const asks = asksOtherAudio({
			unavailable: cta === 'unavailable',
			music: isMusicSubtype(detail?.subtype ?? null),
			refused: availabilityRefused
		});
		noticeOtherAudio = null;
		if (!asks) return;
		return watchOtherAudio(q, (answer) => (noticeOtherAudio = answer));
	});

	// How far the grid renders tiles: with airing data present the
	// grid extends past the provider's playable count to the announced
	// total, so a season the provider hasn't fully listed still shows its
	// unaired tail as greyed dated tiles. Actions keep `episodeCap`.
	const stripCap = $derived(
		displayCap(playableEpisodeCount, detail?.episode_count ?? null, airing)
	);

	/** Padded view of `rawWindowed` for the current page — the same
	 *  reactive shape as the play page's strip. Re-runs when the raw
	 *  fetch lands OR the caps/extras/airing land, so a page fetched
	 *  before availability or the airing schedule resolves still gets
	 *  its placeholder and greyed-unaired tiles: placeholders fill
	 *  episode numbers the provider has but Kitsu doesn't (One Piece:
	 *  Kitsu stops at 1106, the provider has 1161) plus the announced
	 *  unaired tail, and non-integer extras ("1061.5" recaps) splice
	 *  in — `String(number)` round-trips those to the provider fine. */
	const episodes: KitsuEpisode[] | null = $derived.by(() => {
		if (rawWindowed === null) return null;
		const out = rawWindowed.slice();
		const cap = stripCap;
		if (cap !== null) {
			const start = (episodesPage - 1) * UI_PAGE_SIZE + 1;
			const end = episodesPage * UI_PAGE_SIZE;
			const have = new Set(out.map((ep) => ep.number ?? ep.relative_number ?? -1));
			const padTo = Math.min(end, cap);
			for (let n = start; n <= padTo; n++) {
				if (have.has(n)) continue;
				out.push(placeholderEpisode(n));
			}
			for (const tag of extraEpisodes) {
				const n = parseFloat(tag);
				if (!Number.isFinite(n)) continue;
				if (!extraOnPage(n, episodesPage, UI_PAGE_SIZE)) continue;
				out.push(placeholderEpisode(n));
			}
			out.sort(
				(a, b) => (a.number ?? a.relative_number ?? 0) - (b.number ?? b.relative_number ?? 0)
			);
		}
		return out;
	});
	// Number of episodes Kitsu actually indexed for this show. When the
	// user is on page 1 and Kitsu returned fewer than UI_PAGE_SIZE,
	// that's the entire dataset — Kitsu doesn't have more. Different
	// from `detail.episode_count` (the announced total), which can be
	// higher than what Kitsu has actually listed for currently-airing
	// or partially-cataloged seasons (e.g. Re:Zero S4 says 19 but only
	// 5 are indexed).
	const knownAvailableEpisodes = $derived.by(() => {
		if (episodesPage !== 1) return null;
		if (!episodes) return null;
		const whole = wholeEpisodeCount(episodes);
		if (whole < UI_PAGE_SIZE) return whole;
		return null;
	});
	const totalEpisodePages = $derived.by(() => {
		// Display bound (stripCap), not the action cap — the pager must
		// reach the greyed unaired pages the grid renders. If we
		// already know Kitsu only has K (< UI_PAGE_SIZE) episodes,
		// there's nothing on page 2.
		const total = stripCap;
		if (knownAvailableEpisodes !== null) return 1;
		if (!total) return null;
		return Math.max(1, Math.ceil(total / UI_PAGE_SIZE));
	});
	const epStart = $derived((episodesPage - 1) * UI_PAGE_SIZE + 1);
	// Whole episodes only: the recaps beside them are not in the run.
	const pageWhole = $derived(episodes ? wholeEpisodeCount(episodes) : 0);
	const epEnd = $derived((episodesPage - 1) * UI_PAGE_SIZE + pageWhole);

	function kitsuPagesForUiPage(uiPage: number): number[] {
		const start = (uiPage - 1) * UI_PAGE_SIZE + 1;
		const end = uiPage * UI_PAGE_SIZE;
		const first = Math.ceil(start / KITSU_PAGE_SIZE);
		const last = Math.ceil(end / KITSU_PAGE_SIZE);
		const out: number[] = [];
		for (let k = first; k <= last; k++) out.push(k);
		return out;
	}

	async function getKitsuPage(p: number): Promise<KitsuEpisode[]> {
		if (!id) return [];
		const cached = kitsuPageCache.get(p);
		if (cached) return cached;
		const eps = await kitsuEpisodes(id, p);
		kitsuPageCache.set(p, eps);
		return eps;
	}

	async function fetchEpisodesPage(p: number, opts: { initial?: boolean } = {}) {
		if (!id) return;
		const wantPage = Math.max(1, p);
		episodesLoading = true;
		try {
			const start = (wantPage - 1) * UI_PAGE_SIZE + 1;
			const end = wantPage * UI_PAGE_SIZE;
			const merged = (await Promise.all(kitsuPagesForUiPage(wantPage).map(getKitsuPage))).flat();
			rawWindowed = merged.filter((ep) => {
				const n = ep.number ?? ep.relative_number ?? -1;
				return n >= start && n <= end;
			});
			episodesPage = wantPage;
			episodesError = null;
			void prefetchAdjacent(wantPage);
		} catch (e) {
			if (opts.initial) rawWindowed = [];
			episodesError = describeError(e);
		} finally {
			episodesLoading = false;
		}
	}

	/** Synthetic episode record used to fill the gap between Kitsu's
	 *  catalogued range and the provider's playable count. The renderer's
	 *  null-title / null-thumbnail branches already handle missing
	 *  metadata — this just gives the each block a keyed id. */
	function placeholderEpisode(n: number): KitsuEpisode {
		return {
			id: `__placeholder__${n}`,
			number: n,
			relative_number: null,
			season_number: null,
			canonical_title: null,
			synopsis: null,
			airdate: null,
			length: null,
			thumbnail: null
		};
	}

	// Warm the cache for ±1 UI pages so prev/next feels instant. Quietly
	// swallows failures — the user-driven path surfaces errors itself.
	function prefetchAdjacent(uiPage: number) {
		const cap = totalEpisodePages;
		const targets: number[] = [];
		if (cap === null || uiPage + 1 <= cap) targets.push(uiPage + 1);
		if (uiPage - 1 >= 1) targets.push(uiPage - 1);
		for (const t of targets) {
			for (const k of kitsuPagesForUiPage(t)) {
				if (!kitsuPageCache.has(k)) {
					void getKitsuPage(k).catch(() => {});
				}
			}
		}
	}

	function gotoPage(p: number) {
		const cap = totalEpisodePages ?? p;
		const next = Math.min(Math.max(1, p), cap);
		if (next === episodesPage) return;
		// Drive the URL — the `?page=` change wakes the $effect above,
		// which calls fetchEpisodesPage. Calling fetchEpisodesPage
		// directly here would race the effect: the effect re-runs on
		// the resulting `episodesPage` write, sees the URL still on
		// page 1, and immediately fetches page 1 again — which lands
		// just after the page-2 tiles finish their entrance animation,
		// so the UI snaps back to page 1.
		// Build the next query string by hand instead of mutating a
		// URLSearchParams (svelte/prefer-svelte-reactivity flags any
		// mutable instance of the built-in class). It's a one-shot
		// serialization here, so we just iterate the existing params,
		// drop any prior `page=`, and append our new value.
		const parts: string[] = [];
		for (const [k, v] of page.url.searchParams.entries()) {
			if (k === 'page') continue;
			parts.push(`${encodeURIComponent(k)}=${encodeURIComponent(v)}`);
		}
		parts.push(`page=${next}`);
		// goto target IS resolve()-produced; the rule pattern-matches a
		// literal `goto(resolve(...))` call and trips on the template
		// literal that interpolates resolve() with the query string.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		void goto(`${resolve('/anime/[id]', { id })}?${parts.join('&')}`, {
			replaceState: true,
			keepFocus: true,
			noScroll: true
		});
	}

	function jumpToEpisode(event: SubmitEvent) {
		event.preventDefault();
		const n = parseInt(jumpInput, 10);
		if (Number.isNaN(n) || n < 1) return;
		const target = Math.ceil(n / UI_PAGE_SIZE);
		// Set highlightEp first so the effect that watches it picks up the
		// target. If the page changes, the effect re-runs once the new
		// `episodes` array lands; if we're already on the right page, the
		// effect runs on the next microtask.
		highlightEp = n;
		gotoPage(target);
		jumpInput = '';
	}

	// Scroll-to + highlight pulse for the target episode tile. Watches
	// `highlightEp` AND `episodes` so the effect re-runs after the page's
	// fetch completes — that's when the matching <li data-ep-num=…> tile
	// is actually in the DOM. The 3.2s clear timer only starts on the
	// run that successfully finds the tile.
	$effect(() => {
		const target = highlightEp;
		if (target === null) return;
		// Track `episodes` reactively so we re-run after the right page
		// loads. Without this, jumping to an episode on a different
		// page would scroll once before the data lands.
		// eslint-disable-next-line @typescript-eslint/no-unused-expressions
		episodes;

		let timerId: number | undefined;
		const rafId = requestAnimationFrame(() => {
			const el = document.querySelector(`[data-ep-num="${target}"]`);
			if (!el) return; // not rendered yet; effect re-runs when episodes changes
			el.scrollIntoView({ behavior: 'smooth', block: 'center' });
			timerId = window.setTimeout(() => {
				if (highlightEp === target) highlightEp = null;
			}, 3200);
		});

		return () => {
			cancelAnimationFrame(rafId);
			if (timerId !== undefined) clearTimeout(timerId);
		};
	});

	let configError = $state<string | null>(null);

	// The mode as a value rather than as "whatever `config` currently
	// is". Settings resolving from null to `sub` leaves the answer
	// unchanged, and a derived that compares equal does not wake its
	// dependents — so the common case costs one lookup, not two
	// identical ones against a source that rate-limits.
	const availabilityMode = $derived(playMode(config));

	// Availability against the provider. Result gates the Play + Download
	// CTAs so titles outside the catalogue (Western animation, etc.)
	// get a calm notice instead of a click → error overlay (one that
	// names the other audio when the cache has the title there, never
	// for a music title — see $lib/detail/unavailable-copy), and its episode count caps the tiles.
	//
	// Its own effect, reading the mode directly, because the mode
	// arrives after the page does: settings resolve late, so the first
	// lookup goes out with the fallback `sub`, and a sub count says
	// nothing about the dub catalogue. Reading the mode inside the
	// load callback made it invisible to the effect, so nothing
	// reissued the question and the page kept no cap at all — every
	// aired dub tile playable until the user navigated. The play route already worked this way.
	//
	// Network errors leave availability null — the lazy click-failure
	// path then handles it.
	$effect(() => {
		const d = detail;
		const mode = availabilityMode;
		if (!d || isMusicSubtype(d.subtype)) return;
		// Untracked below the two reads above: opening the writeback
		// ticket reads the context, the context reads `config`, and that
		// would put the effect back on `config`'s identity instead of on
		// the mode it resolves to.
		return untrack(() =>
			startAvailabilityLookup(
				{
					title: d.canonical_title,
					altTitles: altTitlesFromKitsu(d),
					episodeCount: d.episode_count ?? undefined,
					year: yearFromKitsuRef(d) ?? undefined,
					kitsuId: d.id,
					status: d.status ?? undefined,
					subtype: d.subtype ?? undefined
				},
				mode,
				{
					check: checkAvailability,
					begin: () => writeback.begin(),
					apply: applyAvailabilityPatch,
					setResolved: (r) => {
						availabilityResolved = r;
						if (r) resumeRefresh.resolved();
						else resumeRefresh.unresolved();
					},
					setListed: (l) => (warmListed = l)
				}
			)
		);
	});

	// The resume row read again once the lookup answered: it paid for
	// the listing that can put the row's numbering right.
	// The warms whose answer has asked the resume row to be read again.
	const warmsAnswered = new WeakSet<Promise<unknown>>();
	/** How long a click waits on the resume row being read again. */
	const RESUME_WAIT_MS = 15_000;
	// Once the row has arrived and the lookup has answered, in either
	// order.
	const resumeRefresh = joinResumeRefresh((entry) => readResumeAgain(entry));
	async function readResumeAgain(entry: HistoryEntry) {
		const showId = id;
		if (!showId) return;
		const atVisit = visit;
		// No measured cap means no listing stood behind the lookup. The
		// warm comes after the row is read, so only for a page still on
		// this visit: a viewer who left meanwhile has had the show's
		// plays released ($lib/play/show-release).
		const warm =
			playableEpisodeCount === null && detail
				? (ep: number) => (!gone && visit === atVisit ? warmResume(ep, showId) : Promise.resolve())
				: undefined;
		const next = await refreshedResume(
			entry,
			showId,
			historyNumber,
			(k, f, t) => movePosition(k, f, t),
			warm
		);
		// The row the refresh read, still the page's: compared by value,
		// since the page keeps its own copy of the row.
		const same = resumeEntry?.id === entry.id && resumeEntry?.ep_no === entry.ep_no;
		if (next && id === showId && same) {
			resumeEntry = next;
			// The next lookup's refresh reads again against this row.
			resumeRefresh.row(next);
		}
	}

	// A resolve of the resume episode that records no watch.
	// `showId` is the show the row was read for, which the play cache
	// is keyed and cleared by.
	function warmResume(ep: number, showId: string) {
		const d = detail;
		const title = playTitle();
		if (!d || !title) return Promise.resolve();
		const mode = playMode(config);
		const quality = playQuality(config);
		// The version a click on the row would play: its warm puts that
		// copy's numbering right and keeps its stream under the key the
		// click reads.
		const version = versionChoice;
		const args = playArgsFor({
			title,
			episode: ep,
			mode,
			quality,
			show: d,
			kitsuId: d.id,
			version
		});
		return warmResumeEpisode({ showId, episode: ep, mode, quality, version }, args, playStream);
	}

	// Synopsis collapse/expand. Default collapsed (long synopses are
	// dominant otherwise); expands on user request.
	let synopsisExpanded = $state(false);

	// Similar titles strip (below the body). Searches Kitsu by the first
	// 1-2 words of the canonical_title and filters out the current anime.
	// Cheap and effective for franchise neighbours; richer
	// recommendations come once AniList wires up (M3+).
	let similar = $state<KitsuAnimeRef[] | null>(null);

	// Inline status banner for an action that reports something without
	// navigating — a refused probe, a title the catalogue does not carry.
	// Kept tight; not a modal.
	let actionNotice = $state<string | null>(null);
	/** Separate state from `actionNotice` so the inline status banner
	 *  (used for transient hints like "downloads land later") doesn't
	 *  share a slot with hard play-call failures. The failure shows in
	 *  a fixed-position overlay regardless of scroll. */
	let playFailure = $state<{ episode: number; message: string } | null>(null);
	// True while a play/playExternal request is in flight. Buttons
	// disable themselves to keep the user from double-clicking into a second resolve
	// into a stack of concurrent spawns.
	let actionBusy = $state(false);
	// A click waiting on the resume row being read again.
	let resumeWaiting = $state(false);
	let actionProgress = $state<string | null>(null);

	// The show's live list entry, folded from EVERY connected tracker so the
	// editor opens on the real status/progress (the deviation safety) and the
	// action-row button reflects it. A Save fans out to ALL connected trackers
	// (see ListEntryEditor → set-entry), so we must seed from all of them too:
	// reading only one provider that lacks the title would seed Add/Planning/0
	// and let a Save clobber another tracker that already has progress.
	// pickSeedEntry keeps the furthest-along entry. Reloaded when the show id
	// or the connected set changes.
	let listEntry = $state<EntryView | null>(null);
	// True while the providers' live entries are being read. The editor stays
	// disabled in this window so a Save can't fire against a not-yet-known
	// entry (which would seed Planning/0 and overwrite the real status).
	let listEntryLoading = $state(false);
	// True when the read couldn't establish the real tracker state (a bearer
	// couldn't be refreshed, or a getEntry threw). A null listEntry then means
	// "unknown", not "not on the list", so the editor stays disabled rather
	// than offering an Add that could overwrite an existing entry. Any single
	// provider failing trips this — a partial read could miss the very tracker
	// whose progress a Save would otherwise clobber.
	let listEntryError = $state(false);
	// Why the control is disabled, when the read ended on something worth
	// saying: the tracker's rate limit and the wait it asked for. Null for
	// a plain failure, which disables the control as before.
	let listEntryNotice = $state<string | null>(null);
	// Bumped on every read so a newer read (navigation or a post-write
	// reconcile) supersedes any in-flight one — including one waiting
	// out a retry, which checks the sequence again when its wait ends.
	let listEntryReadSeq = 0;

	// Read every connected tracker and fold to the seed entry, retrying a
	// transient failure on the plan in `read-retry` so a passing failure
	// self-heals; the loop itself is `readEntryWithRetry`. Two modes:
	//  - loud (navigation): reset to null + show loading + disable on hard
	//    failure, so a Save can't fire against a not-yet-known entry.
	//  - quiet (post-write reconcile): refresh in the background without a
	//    loading/disabled flicker; on failure keep the optimistic value (the
	//    write already toasted) and let the next read reconcile.
	function readListEntry(quiet: boolean) {
		const seq = ++listEntryReadSeq;
		const kitsuId = id;
		const connected = accountStore.connected;
		if (!quiet) {
			// The route reuses this component across shows; a stale entry must not
			// linger while the async read is in flight or if it fails.
			listEntry = null;
			listEntryError = false;
			listEntryNotice = null;
		}
		if (!kitsuId || connected.length === 0) {
			if (!quiet) listEntryLoading = false;
			return;
		}
		if (!quiet) listEntryLoading = true;
		const providers = [...connected];
		void readEntryWithRetry({
			// Promise.all rejects on the first failure, so any unreadable
			// provider ends the attempt rather than seeding from a partial
			// picture.
			readAll: () =>
				Promise.all(
					providers.map(async (p) => {
						const bearer = await freshBearerFor(p);
						if (!bearer) throw new Error(`no bearer for ${p}`);
						return getEntry(p, bearer, kitsuId);
					})
				),
			isCurrent: () => seq === listEntryReadSeq,
			sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms))
		}).then((outcome) => {
			if (outcome.kind === 'superseded') return;
			if (outcome.kind === 'ok') {
				listEntry = outcome.entry;
				listEntryError = false;
				listEntryNotice = null;
				listEntryLoading = false;
				return;
			}
			// Out of retries. On a fresh read: unknown entry, not absent — the
			// editor stays disabled rather than offering a stale Add, and says
			// why when the tracker's rate limit is the reason. Quiet: give up
			// silently, keep whatever the editor optimistically set; the write
			// itself already reported success/failure.
			if (!quiet) {
				listEntryError = true;
				listEntryNotice = readFailureNotice(outcome);
				listEntryLoading = false;
			}
		});
	}

	$effect(() => {
		// Track id + the connected set; re-read (loud) whenever they change.
		void id;
		void accountStore.connected;
		readListEntry(false);
		return () => {
			listEntryReadSeq += 1; // cancel any in-flight read on teardown
		};
	});

	// Episodes-fallback derivations live in script so they can be used in
	// the markup without {@const} (which only accepts Svelte-block parents).
	const epPlaceholderCount = $derived(
		detail?.episode_count ? Math.min(UI_PAGE_SIZE, detail.episode_count) : UI_PAGE_SIZE
	);
	const showEpPlaceholders = $derived(episodes !== null && episodes.length === 0);

	/** Tile label for an unaired episode: air date on the very next
	 *  one, the generic "Unaired" otherwise. */
	function unairedLabel(airsAt: number | null): string {
		return airsAt
			? m.detail_ep_airs({ date: formatAirDate(airsAt, getLocale()) })
			: m.detail_ep_unaired();
	}

	// Labels are message functions, called at render, so a locale
	// switch relabels the buttons without rebuilding the table.
	const QUALITIES: Array<{ key: string; label: () => string }> = [
		{ key: 'best', label: m.app_quality_best },
		{ key: '1080', label: () => '1080' },
		{ key: '720', label: () => '720' },
		{ key: '480', label: () => '480' },
		{ key: 'worst', label: m.app_quality_worst }
	];

	// Cancel the plays of every show this page showed on unmount —
	// warms and clicks alike. Prevents abandoned resolves from holding
	// provider slots after the user navigates away — clearForShow
	// aborts each entry's signal which closes the SSE EventSource and
	// rejects the promise.
	// Each show is recorded as the route names it: by the time the
	// page is destroyed the route names where the user went
	// ($lib/play/show-release).
	const showReleases = createShowReleases(clearForShow);
	$effect.pre(() => showReleases.showing(() => id));
	onDestroy(showReleases.leave);

	// Re-fetch detail + similar when the URL `id` changes. SvelteKit
	// reuses this component for /anime/[a] → /anime/[b] navigations
	// without remounting, so onMount alone wouldn't refire — the
	// "Similar titles" cards used to refresh the page silently with
	// stale data. Reset render state at the top so we don't briefly
	// show the prior show's hero before the new fetch lands.
	$effect(() => {
		const currentId = id;
		if (!currentId) {
			error = { headline: m.detail_error_no_id_headline(), detail: m.detail_error_no_id_detail() };
			return;
		}
		detail = null;
		rawWindowed = null;
		episodesError = null;
		similar = null;
		error = null;
		availabilityRow = null;
		availabilityResolved = false;
		warmListed = null;
		showVersions = [];
		versionChoice = storedChoice(currentId);
		resumeEntry = null;
		resumeRefresh.reset();
		resetEpisodePageCache(kitsuPageCache);
		const detailServed = kitsuAnimeDetail(currentId);
		void lookupResume(currentId, historyByKitsu, detailServed)
			.then((h) => {
				if (id !== currentId) return;
				resumeEntry = h;
				resumeRefresh.row(h);
			})
			.catch(() => {
				// Resume is a hint, not load-bearing — drop silently.
			});
		void detailServed
			.then((d) => {
				if (id !== currentId) return; // navigation raced ahead
				detail = d;
				if (isMusicSubtype(d.subtype)) {
					// Music videos (a YOASOBI MV, say) never exist on the provider —
					// it indexes anime episodes, not song clips. Mark the show
					// unavailable up front instead of letting the probe fuzzy-match
					// an unrelated anime and play the wrong show. The lookup
					// effect below stands down for these.
					availabilityResolved = true;
				}
				// Override the layout's URL-only default with the
				// loaded title so the breadcrumb reads the show
				// instead of "Anime".
				breadcrumb.set([
					{ label: m.breadcrumb_home(), href: '/' },
					{ label: d.canonical_title ?? m.breadcrumb_anime() }
				]);
				const seed = (d.canonical_title ?? '').split(/\s+/).slice(0, 2).join(' ').trim();
				if (seed.length >= 2) {
					// Pipe similar-titles through the same availability filter
					// the home rows use. Cache-only / fire-and-forget — drops
					// kitsu hits we already know aren't on the streaming source,
					// warms uncached entries for the next visit. Without this
					// the strip happily renders dead shows whose detail pages
					// only dead-end on the Watch click.
					void kitsuSearch(seed)
						.then((hits) => {
							if (id !== currentId) return null;
							const trimmed = hits.filter((h) => h.id !== d.id).slice(0, 12);
							const mode: 'sub' | 'dub' = playMode(config);
							return filterAvailable(trimmed, mode);
						})
						.then((filtered) => {
							if (id !== currentId || filtered === null) return;
							similar = filtered;
						})
						.catch(() => {
							similar = [];
						});
				} else {
					similar = [];
				}
			})
			.catch((e) => {
				if (id !== currentId) return;
				error = { headline: m.detail_error_load_headline(), detail: describeError(e) };
			});
		// The component outlives a move to another show, so the show
		// left drops its warms here — nobody is waiting on them. A
		// click's own play runs on and records its show's watch. Only
		// when the id moved: this effect also re-runs on the same show.
		return () => {
			if (id !== currentId) clearBackgroundForShow(currentId);
		};
	});

	onMount(() => {
		void settingsGet()
			.then((c) => (config = c))
			.catch((e) => (configError = describeSettingsFailure(e)));
	});

	// Background prefetch: as soon as we have the show title + the
	// user's current mode/quality, fire a play() for the default
	// episode in the background so the eventual hero-button click is
	// instant. The play-cache dedupes against the click handler — both
	// share the same promise, so there's never a duplicate
	// spawn. Failures are swallowed here; the click handler surfaces
	// them when the user actually wants to play.
	$effect(() => {
		const title = detail?.canonical_title;
		if (!id || !title || !config) return;
		// Music videos are not in the provider's catalogue; prefetching one resolves
		// with the music title and fuzzy-matches an unrelated show, warming the
		// play cache with the wrong candidate. The Play/Download UI is already
		// gated off for these (availability=false), so skip the warm too.
		if (isMusicSubtype(detail?.subtype ?? null)) return;
		// Wait for the airing answer before warming anything — firing
		// on mount with airing still unknown would resolve the whole
		// visible schedule, greyed-out future episodes included. airingPending (not raw
		// airingResolved) so a show without a Kitsu status — where no
		// airing fetch ever starts — doesn't deadlock the warm.
		if (airingIsPending) return;
		// ...and for the availability probe: until it settles,
		// playableEpisodeCount is null and beyondPlayable reads the
		// cap as unbounded, so the warm would resolve
		// aired-but-uncatalogued padded tiles.
		if (!availabilityResolved) return;
		const mode = playMode(config);
		const quality = playQuality(config);
		// Read here so a new choice re-warms under its own keys; the
		// other version's warmed sessions are never served for it.
		const version = versionChoice;

		// Fan out a prefetch for every visible AIRED episode tile in
		// the strip. The play-cache dedupes simultaneous calls —
		// clicking any tile while the strip is still resolving shares
		// the same in-flight promise. The default-episode prefetch
		// (ep 1) falls out of the same loop; if `episodes` hasn't
		// loaded yet we still warm ep 1 so the hero "Play" button is
		// instant.
		// Aired per AniList AND catalogued by the provider — the grid can
		// render aired-but-uncatalogued tiles (displayCap), and warming
		// those is the same doomed resolution the availability probe
		// already capped out.
		// Which tiles to warm — and how wide, given the caching
		// setting — is the module's decision; the page only feeds it
		// the runtime inputs.
		const targets = detailWarmTargets({
			cacheResolutions: config.cache_resolutions,
			visible: episodes ? episodes.map((e) => e.number ?? e.relative_number ?? null) : null,
			heroEpisode: defaultEpisode(),
			airing,
			playableCount: playableEpisodeCount,
			listed: warmListed
		});
		for (const ep of targets) {
			// Built now, not when the warm's slot frees up: a warm queued
			// behind others must describe the show it was queued for even
			// if the page has moved to another by then. A prefetch must
			// NOT update Continue Watching — the calls resolve in arbitrary
			// order, so whichever finished last would overwrite the user's
			// actual click. The Kitsu id rides along so the backend can
			// record the (provider show_id → kitsu_id) reverse mapping even
			// if the user only ever prefetches.
			const args = playArgsFor({
				title,
				episode: ep,
				mode,
				quality,
				show: detail,
				kitsuId: id,
				prefetch: true,
				version
			});
			const warmedVisit = visit;
			const warm = getOrFire(makeKey(id, ep, mode, quality, version), (emit, signal) =>
				playStream(args, emit, signal)
			);
			// The warm paid for a listing: the resume row is read again —
			// once per warm, not on every re-run that finds it cached.
			if (!warmsAnswered.has(warm)) {
				warmsAnswered.add(warm);
				void warm.then(
					() => {
						if (visit === warmedVisit) resumeRefresh.again();
					},
					() => {}
				);
			}
			void warm.catch(() => {
				/* the click handler will see the error if it ever fires;
				 *  unmount-cancel rejects with "aborted" which we swallow */
			});
		}
	});

	// Drive the episode page off the URL ?page= param. Re-runs on
	// every URL change so navigation between two /anime/[id] entries
	// with different query strings works (SvelteKit reuses the
	// component when the route id is the same; a plain onMount fires
	// only once). The decision rule lives in $lib/history/url-deeplink.
	$effect(() => {
		if (!id) return;
		const targetPage = parsePageParam(page.url.searchParams);
		const action = decideEpisodeFetchAction({
			episodes,
			episodesPage,
			episodesLoading,
			targetPage
		});
		if (action === 'fetch-initial') {
			void fetchEpisodesPage(targetPage, { initial: true });
		} else if (action === 'fetch') {
			void fetchEpisodesPage(targetPage);
		}
	});

	$effect(() => {
		const onScroll = () => {
			scrollY = window.scrollY;
		};
		window.addEventListener('scroll', onScroll, { passive: true });
		onScroll();
		return () => window.removeEventListener('scroll', onScroll);
	});

	/** Play-call failure copy: the shared mapper with this surface's
	 *  one deliberate difference — the definitive catalogue-miss
	 *  phrasing (this page also gates the Play CTA proactively) — and
	 *  the other audio's answer from the availability cache. */
	function describePlayFailure(
		e: unknown,
		kitsuId: string,
		mode: string,
		episode: number
	): Promise<string> {
		return describePlayFailureFromCache(
			e,
			{ kitsuId, mode, episode },
			{ noResults: m.detail_error_play_no_results }
		);
	}

	// Hero framing preferences — see $lib/hero/prefs. A choice made in
	// the switch applies to the hero at once and is kept for the next load.
	let heroPrefs = $state(loadHeroPrefs(browserStorage()));
	function chooseHeroPrefs(next: HeroPrefs) {
		heroPrefs = next;
		saveHeroPrefs(browserStorage(), next);
	}

	// `raw` is the upstream URL `url` proxies; a load failure is noted
	// against it, and the next pick passes it over.
	function heroFor(d: KitsuAnimeRef): {
		url: string | null;
		raw: string | null;
		isCover: boolean;
		origin: HeroOrigin | null;
	} {
		// `original` last as defense — backend's eager-warm caches
		// signed-URL bytes under a canonical hash so the proxy serves
		// them regardless of staleness. See PosterCard for full
		// rationale. (The slot order lives in $lib/hero/framing.)
		const p = pickHeroImage(d, heroPrefs, 'detail', failedHeroImages);
		return { url: imageProxyUrl(p.url), raw: p.url, isCover: p.isCover, origin: p.origin };
	}

	// Natural aspect ratio per show, once its hero image decodes — the
	// never-crop framing sizes the image box from it.
	let heroRatios = $state<Record<string, number>>({});
	function onHeroLoad(id: string, e: Event) {
		const img = e.currentTarget as HTMLImageElement;
		const r = naturalRatio(img.naturalWidth, img.naturalHeight);
		if (r !== null) heroRatios[id] = r;
	}
	function posterFor(d: KitsuAnimeRef): string | null {
		return imageProxyUrl(
			d.poster_image?.large ?? d.poster_image?.medium ?? d.poster_image?.original ?? null
		);
	}
	function ratingDisplay(r: number | null): string | null {
		if (r === null) return null;
		return (r / 10).toFixed(1);
	}
	function yearOf(d: KitsuAnimeRef): string | null {
		return d.start_date ? d.start_date.slice(0, 4) : null;
	}
	function statusLabel(s: string | null): string {
		if (!s) return m.detail_status_unknown();
		if (s === 'current') return m.detail_status_current();
		if (s === 'finished') return m.detail_status_finished();
		if (s === 'upcoming') return m.detail_status_upcoming();
		return s;
	}
	function heroTransform(y: number, isCover: boolean): string {
		// Honor prefers-reduced-motion: when set, the hero doesn't translate
		// on scroll. Scale (which doesn't move) is kept for visual polish.
		if (
			typeof window !== 'undefined' &&
			window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
		) {
			return `translate3d(0, 0, 0) scale(${isCover ? 1.02 : 1.15})`;
		}
		const offset = Math.min(y * 0.25, 80);
		const scale = isCover ? 1.02 : 1.15;
		return `translate3d(0, ${offset}px, 0) scale(${scale})`;
	}

	async function setMode(mode: 'sub' | 'dub') {
		if (!config || config.mode === mode) return;
		const next: Config = { ...config, mode };
		config = next;
		try {
			await settingsPut(next);
		} catch (e) {
			configError = describeSettingsFailure(e);
		}
	}
	async function setQuality(q: string) {
		if (!config || config.quality === q) return;
		const next: Config = { ...config, quality: q };
		config = next;
		try {
			await settingsPut(next);
		} catch (e) {
			configError = describeSettingsFailure(e);
		}
	}

	function notify(msg: string) {
		actionNotice = msg;
		// auto-dismiss after a beat so it doesn't stack.
		setTimeout(() => {
			if (actionNotice === msg) actionNotice = null;
		}, 4000);
	}

	// Title we feed to the provider's search. The backend searches on
	// it and then disambiguates the results itself — `pick_candidate`
	// weighs episode count, year, subtype and title — so what this
	// side owes it is a stable, canonical query rather than a lucky
	// one. KitsuAnimeRef.canonical_title is non-null
	// per the type, but the detail isn't populated until kitsuAnimeDetail
	// resolves — guard for the null pre-load state.
	function playTitle(): string {
		return detail?.canonical_title ?? '';
	}

	/** Hero CTA target. When the user has watched this show before
	 *  (resumeEntry populated from the reverse cache), point at the
	 *  episode AFTER the one they last watched, capped at the show's
	 *  known episode_count so we don't propose a non-existent ep.
	 *  Falls back to episode 1. */
	function defaultEpisode(): number {
		// Delegated to the shared helper so the home Continue Watching
		// card and this CTA compute the same answer from the same
		// inputs: no resume entry → 1, mid-show → last + 1, at cap →
		// last (Replay branch). The cap clamps to the aired count so a
		// user watched through the aired episodes gets Replay of the
		// last aired one instead of Continue into an unaired episode.
		const last = resumeEntry ? parseHistoryEpisode(resumeEntry.ep_no) : null;
		return pickResumeEpisode(last, airedCap(episodeCap, airing), leftPartWay(last));
	}

	/** Whether the last watched episode was left part-way, its position
	 *  kept: Play goes back to it instead of on to the next. */
	function leftPartWay(last: number | null): boolean {
		return last !== null && Number.isFinite(last) && readPosition(id, last) !== null;
	}

	/** Label for the primary action button. Five-state machine
	 *  resolved by `computePlayLabel`: single-video shows (movies,
	 *  one-episode finished OVAs/specials) read "Watch" / "Watch
	 *  again" with no episode number; multi-episode shows read
	 *  "Play episode 1" / "Continue · Episode N" / "Replay ·
	 *  Episode N" based on the resume entry and the capped
	 *  defaultEpisode. */
	const playLabel = $derived.by(() => {
		const state = computePlayLabel({
			isSingleVideo: singleVideo,
			resumeEntry,
			defaultEpisode: defaultEpisode(),
			leftPartWay: leftPartWay(resumeEntry ? parseHistoryEpisode(resumeEntry.ep_no) : null)
		});
		switch (state.kind) {
			case 'watch':
				return m.detail_play_button_watch();
			case 'watch_again':
				return m.detail_play_button_watch_again();
			case 'episode_one':
				return m.detail_play_button_episode_one();
			case 'resume':
				return m.detail_play_button_resume({ episode: String(state.episode) });
			case 'replay':
				return m.detail_play_button_replay({ episode: String(state.episode) });
			case 'continue':
				return m.detail_play_button_continue();
		}
	});

	async function startPlay(ep: number) {
		const title = playTitle();
		if (!title) {
			notify(m.detail_notify_no_title());
			return;
		}
		const mode = playMode(config);
		const quality = playQuality(config);
		const version = versionChoice;
		// The show and visit this play is for, and its request, taken at
		// the click ($lib/play/owned-play): a late answer is judged
		// against them, and records the show the viewer clicked.
		const here = navigationClaims.claim();
		const owner = claimPlay(
			id,
			() => visit,
			() => gone || !here()
		);
		const show = detail;
		const playArgs = playArgsFor({
			title,
			episode: ep,
			mode,
			quality,
			show,
			kitsuId: owner.showId,
			version
		});
		// The resolve records the watch; mark the episode started first,
		// so Continue stays on it even if the player never opens.
		markStarted(owner.showId, ep);
		// LoadingOverlay binds to actionBusy; it stays up until goto
		// fires (which unmounts this page), until a failure's message is
		// worded (it waits on a cache lookup, so no second play starts
		// under a failure that has not landed), or until the answer turns
		// out to belong to a visit the viewer left.
		actionBusy = true;
		actionNotice = null;
		actionProgress = null;
		// Hits the play-cache: a prefetch from onMount or an earlier
		// click against the same (show, ep, mode, quality, version)
		// completes instantly here. The streaming variant feeds progress
		// events into the overlay so the user sees `<provider> ✓` ticks.
		const outcome = await runOwnedPlay(
			owner,
			() =>
				getOrFire(
					makeKey(owner.showId, ep, mode, quality, version),
					(emit, signal) => playStream(playArgs, emit, signal),
					(p) => {
						actionProgress = progressLabel(p);
					}
				),
			(e) => describePlayFailure(e, owner.showId, mode, ep)
		);
		if (outcome.kind === 'played') {
			actionNotice = null;
			// Stamp Continue Watching for the click. getOrFire shares
			// the prefetch's in-flight promise (which had prefetch=true),
			// so the backend's auto-history-write stayed silent for
			// cache-hit clicks — markWatched covers that gap. The backend
			// op is idempotent.
			// Completion is decided against the FULL finite series total
			// (Kitsu's mode-independent count), NOT episodeCap — in dub
			// mode the playable cap is only the dubbed slice — and only for a finished series.
			// The episode the session plays: the request may have named it
			// by a number it no longer carries.
			const played = followPlayed(owner.showId, outcome.session, ep);
			recordPlayed(
				{
					args: { ...playArgs, episode: String(played) },
					showId: owner.showId,
					episode: played,
					show,
					mirror: true
				},
				{ markWatched, syncTrackers: syncWatchedToTrackers }
			);
			if (outcome.current) {
				// `kind` rides along so the player page knows whether to
				// mount hls.js or a plain `<video src>`. The no-resolve
				// lint rule only recognises a literal `goto(resolve(...))`.
				/* eslint-disable svelte/no-navigation-without-resolve */
				void goto(
					resolve('/play/[id]', { id: owner.showId }) + buildPlayQuery(outcome.session, played)
				);
				/* eslint-enable svelte/no-navigation-without-resolve */
				return;
			}
		}
		actionBusy = false;
		actionProgress = null;
		// Fixed-position overlay rather than an inline banner — the
		// user just clicked their primary action and it failed, scrolled
		// out of view of the page header, so the message needs to follow
		// them. They dismiss explicitly. An abandoned failure says
		// nothing: it is about a visit the viewer left.
		if (outcome.kind === 'failed') playFailure = { episode: ep, message: outcome.message };
	}

	// Not-yet-premiered shows can exist as searchable provider stubs,
	// so availability === true while zero episodes have aired. The
	// tiles and prefetch are gated by epAirState, but pickNextEpisode's
	// no-history branch returns 1 regardless of the cap — block the
	// primary CTA too.
	const nothingAiredYet = $derived(airedCap(episodeCap, airing) === 0);

	/** Waits, busy, while the resume row is being read again — it is
	 *  the one a click goes on from — for at most a bound; whether the
	 *  click still belongs to this visit after. */
	async function resumeRowSettled(): Promise<boolean> {
		const atVisit = visit;
		// A navigation away started during the wait leaves the click behind
		// too: the play claims afresh after it, too late to notice.
		const here = navigationClaims.claim();
		const current = () => visit === atVisit && !gone && here();
		resumeWaiting = true;
		try {
			await settledWithin(resumeRefresh, RESUME_WAIT_MS, current);
		} finally {
			resumeWaiting = false;
		}
		// Another action started meanwhile owns the page now.
		return current() && !actionBusy;
	}

	async function onPlay() {
		if (nothingAiredYet || airingIsPending) return;
		if (resumeRefresh.refreshing() && !(await resumeRowSettled())) return;
		if (nothingAiredYet || airingIsPending) return;
		void startPlay(defaultEpisode());
	}
	// Download flow — opens DownloadConfirm modal. The dialog lets the
	// user pick a folder (defaulting to the backend's download_dir
	// resolver) before kicking off the downloader. Active downloads then
	// surface in the global topbar dock + bottom progress strip.
	let downloadModalOpen = $state(false);
	let downloadArgs = $state<DownloadArgs | null>(null);
	let downloadDefaultDir = $state('');
	void downloadDefaultDirApi()
		.then((d) => {
			if (d) downloadDefaultDir = d;
		})
		.catch(() => {});

	async function onDownload() {
		if (!detail || nothingAiredYet || airingIsPending) return;
		if (resumeRefresh.refreshing() && !(await resumeRowSettled())) return;
		if (!detail || nothingAiredYet || airingIsPending) return;
		const mode = playMode(config);
		const quality = playQuality(config);
		// `episode_count` must stay Kitsu's announced total — that's
		// what the backend picker compares against the provider's planned
		// `episodeCount`. The modal's range cap (the provider's released-
		// so-far count) is a separate concern, passed via DownloadConfirm's
		// `availableEpisodes` prop.
		downloadArgs = buildDownloadArgs({
			detail,
			episode: defaultEpisode(),
			mode,
			quality,
			kitsuId: id,
			version: versionChoice
		});
		downloadModalOpen = true;
	}
	function onPickEpisode(n: number) {
		// Honour the availability gate — episode tiles call this too,
		// not just the masthead Play CTA. Without this guard a click
		// on a tile fires startPlay → resolve → NoResults overlay.
		if (availability === false) return;
		void startPlay(n);
	}
</script>

<svelte:head>
	<title
		>{m.app_page_title_detail({
			title: detail?.canonical_title ?? m.app_page_title_loading()
		})}</title
	>
</svelte:head>

<main class="page" style:--accent={accent}>
	{#if error}
		<div class="state state-error" role="alert">
			<p class="state-headline">{error.headline}</p>
			{#if error.detail}<p class="state-detail">{error.detail}</p>{/if}
		</div>
	{:else if detail === null}
		<!-- Skeleton -->
		<section class="hero hero-skeleton" aria-busy="true">
			<div class="hero-img hero-skeleton-img"></div>
		</section>
		<div class="content">
			<section class="masthead">
				<div class="poster-frame poster-skeleton"></div>
				<div class="masthead-text">
					<div class="line line-skeleton" style="inline-size: 70%; block-size: 2.5rem;"></div>
					<div class="line line-skeleton" style="inline-size: 40%; block-size: 1rem;"></div>
					<div class="line line-skeleton" style="inline-size: 90%; block-size: 0.8rem;"></div>
					<div class="line line-skeleton" style="inline-size: 80%; block-size: 0.8rem;"></div>
					<div class="line line-skeleton" style="inline-size: 60%; block-size: 0.8rem;"></div>
				</div>
			</section>
		</div>
	{:else}
		{@const hero = heroFor(detail)}
		{@const heroId = detail.id}
		{@const poster = posterFor(detail)}
		{@const rating = ratingDisplay(detail.average_rating)}
		{@const year = yearOf(detail)}

		<section
			class="hero"
			class:hero-fallback={!hero.isCover}
			class:framing-contain={hero.isCover && heroPrefs.framing === 'contain'}
			class:framing-pillar={hero.isCover && heroPrefs.framing === 'pillar'}
			class:framing-focus={hero.isCover && heroPrefs.framing === 'focus'}
		>
			{#if hero.url && hero.isCover && (heroPrefs.framing === 'contain' || heroPrefs.framing === 'pillar')}
				<img
					class="hero-backdrop"
					src={hero.url}
					alt=""
					style:transform={`${heroTransform(scrollY, true)} scale(1.22)`}
				/>
			{/if}
			{#if hero.url}
				<img
					class="hero-img"
					src={hero.url}
					alt=""
					style:transform={heroTransform(scrollY, hero.isCover)}
					style:--ratio={heroRatios[heroId]}
					onload={(e) => onHeroLoad(heroId, e)}
					onerror={() => hero.raw && noteHeroImageFailed(hero.raw)}
				/>
			{/if}
			<div class="hero-scrim" aria-hidden="true"></div>
			{#if !hero.isCover}
				<div class="hero-grain" aria-hidden="true"></div>
			{/if}
			{#if heroPrefs.controls}
				<HeroFramingSwitch prefs={heroPrefs} origin={hero.origin} onchoose={chooseHeroPrefs} />
			{/if}
		</section>

		<div class="content">
			<section class="masthead">
				<div class="poster-frame">
					{#if poster}
						<img class="poster-img" src={poster} alt="" />
					{:else}
						<span class="poster-placeholder" aria-hidden="true">
							<span class="poster-placeholder-title">{detail.canonical_title}</span>
						</span>
					{/if}
				</div>

				<div class="masthead-text">
					<p class="eyebrow">
						<span class="eyebrow-key">{subtypeLabel(detail.subtype)}</span>
						<span class="eyebrow-rule" aria-hidden="true"></span>
						<span class="eyebrow-value">{statusLabel(detail.status)}</span>
					</p>

					<h1 class="title">{detail.canonical_title}</h1>

					<!-- Action row: primary play, secondary download. Gated
					     by the availability probe — until it lands, render
					     a placeholder so we never flash CTAs labeled with
					     stale Kitsu counts. False → the unavailable notice
					     ("not in catalogue", or the other audio when the cache
					     has the title there and it is not a music title). True → real Play/Download with the
					     provider-truth count threaded through. -->
					<!-- Availability-specific, non-editor content: a loading skeleton
					     while the probe is in flight; the unavailable notice when the
					     show can't be streamed in the chosen audio. -->
					{#if cta === 'loading'}
						<div
							class="actions actions-loading"
							aria-label={m.detail_actions_aria_label()}
							aria-busy="true"
						>
							<span class="action-skel"></span>
							<span class="action-skel action-skel-narrow"></span>
						</div>
					{:else if cta === 'unavailable'}
						<p class="unavailable" role="status">
							<span aria-hidden="true">⏵</span>
							{unavailableMessage(availabilityMode, noticeOtherAudio)}
						</p>
					{/if}

					<!-- The actions row: Play/Download (only when the show is streamable)
					     plus the tracker list editor. The editor needs only the Kitsu id
					     + provider mapping, so a connected user can manage a list entry
					     even for a title the provider can't stream — and it's rendered exactly
					     ONCE here, never once per availability branch, so an availability
					     flip can't remount it and drop an in-flight save or a pending
					     partial-save retry. -->
					{#if cta === 'ready' || (accountStore.hasAny && detail)}
						<div class="actions" aria-label={m.detail_actions_aria_label()}>
							{#if cta === 'ready'}
								<button
									type="button"
									class="btn btn-glass"
									style:--btn-glow="var(--accent)"
									onclick={onPlay}
									disabled={actionBusy || resumeWaiting || nothingAiredYet || airingIsPending}
									title={nothingAiredYet ? m.detail_ep_unaired_tooltip() : undefined}
								>
									<span aria-hidden="true">▸</span>
									<span
										>{nothingAiredYet
											? unairedLabel(airing?.next_airing_at ?? null)
											: playLabel}</span
									>
								</button>
								<button
									type="button"
									class="btn btn-outline"
									onclick={onDownload}
									disabled={nothingAiredYet || airingIsPending}
									title={nothingAiredYet ? m.detail_ep_unaired_tooltip() : undefined}
								>
									<span aria-hidden="true">↓</span>
									<span>{m.detail_download_button()}</span>
								</button>
							{/if}
							{#if accountStore.hasAny && detail}
								<ListEntryEditor
									kitsuId={id}
									total={detail.episode_count ?? null}
									cap={episodeCap}
									current={listEntry}
									airing={detail.status != null && detail.status !== 'finished'}
									disabled={listEntryLoading || listEntryError}
									notice={listEntryNotice}
									onReconcile={() => readListEntry(true)}
								/>
							{/if}
						</div>
					{/if}

					<!-- Sub/Dub + Quality controls. Reads/writes ani-gui config. -->
					<div class="controls">
						<div class="seg-group" role="group" aria-label={m.detail_audio_group_aria_label()}>
							<span class="seg-label">{m.detail_audio_group_label()}</span>
							<div class="seg">
								{#each ['sub', 'dub'] as mode (mode)}
									<button
										type="button"
										class="seg-btn"
										class:active={config?.mode === mode}
										aria-pressed={config?.mode === mode}
										disabled={!config}
										onclick={() => setMode(mode as 'sub' | 'dub')}
									>
										{mode.toUpperCase()}
									</button>
								{/each}
							</div>
						</div>

						<div class="seg-group" role="group" aria-label={m.detail_quality_group_aria_label()}>
							<span class="seg-label">{m.detail_quality_group_label()}</span>
							<div class="seg seg-narrow">
								{#each QUALITIES as q (q.key)}
									<button
										type="button"
										class="seg-btn"
										class:active={config?.quality === q.key}
										aria-pressed={config?.quality === q.key}
										disabled={!config}
										onclick={() => setQuality(q.key)}
									>
										{q.label()}
									</button>
								{/each}
							</div>
						</div>

						{#if offeredVersions.length > 0}
							<!-- Shown only for a show the provider lists twice. A
							     choice of one of N, so a radio group: one tab stop,
							     the arrows move the choice. -->
							<div class="seg-group">
								<span class="seg-label">{m.detail_version_group_label()}</span>
								<div class="seg" role="radiogroup" aria-label={m.detail_version_group_aria_label()}>
									{#each offeredVersions as v, i (v.kind)}
										<button
											type="button"
											class="seg-btn"
											class:active={shownVersion === v.kind}
											role="radio"
											aria-checked={shownVersion === v.kind}
											tabindex={shownVersion === v.kind ? 0 : -1}
											onclick={() => chooseVersion(v.kind)}
											onkeydown={(e) => onVersionKey(e, i)}
										>
											{versionLabel(v.kind)}
										</button>
									{/each}
								</div>
							</div>
						{/if}

						{#if configError}
							<span class="seg-error" role="alert"
								>{m.detail_settings_error_prefix({ reason: configError })}</span
							>
						{/if}
					</div>

					<ul class="meta-row" aria-label={m.detail_meta_row_aria_label()}>
						{#if year}
							<li class="meta-pill">
								<span class="meta-key">{m.detail_meta_year_key()}</span>
								<span class="meta-val num">{year}</span>
							</li>
						{/if}
						{#if detail.episode_count}
							<li class="meta-pill">
								<span class="meta-key">{m.detail_meta_episodes_key()}</span>
								<span class="meta-val num">{detail.episode_count}</span>
							</li>
						{/if}
						{#if rating}
							<li class="meta-pill">
								<span class="meta-key">{m.detail_meta_rating_key()}</span>
								<span class="meta-val num">
									<span class="star" aria-hidden="true">★</span>{rating}<span class="meta-faint"
										>{m.detail_meta_rating_suffix()}</span
									>
								</span>
							</li>
						{/if}
						{#if detail.age_rating}
							<li class="meta-pill">
								<span class="meta-key">{m.detail_meta_age_key()}</span>
								<span class="meta-val">{detail.age_rating}</span>
							</li>
						{/if}
						{#if detail.popularity_rank}
							<li class="meta-pill">
								<span class="meta-key">{m.detail_meta_rank_key()}</span>
								<span class="meta-val num">#{detail.popularity_rank}</span>
							</li>
						{/if}
					</ul>
				</div>
			</section>

			{#if actionNotice}
				<div class="action-notice" role="status">
					<span class="action-notice-key">{m.detail_action_notice_key()}</span>
					<span class="action-notice-rule" aria-hidden="true"></span>
					<span>{actionNotice}</span>
				</div>
			{/if}

			<!-- Body: synopsis + episodes stacked vertically. The previous
		     side-by-side layout looked unbalanced when one was much taller
		     than the other (long synopsis + 12-ep show, or short synopsis
		     + 1100-ep show). Stacked, both panels use the full editorial
		     column width and visually breathe. -->
			<section class="body">
				<div class="body-col body-col-prose">
					<h2 class="section-eyebrow">{m.detail_synopsis_heading()}</h2>
					{#if detail.synopsis}
						<div class="prose-wrap" class:expanded={synopsisExpanded}>
							<p class="prose">{detail.synopsis}</p>
							{#if detail.synopsis.length > 360}
								<!-- Only render the fade when the prose actually clips
								     past the 9.5rem max-block-size — short synopses fit
								     in the box and the gradient over empty space reads
								     as a stray blur. -->
								<div class="prose-fade" aria-hidden="true"></div>
							{/if}
						</div>
						{#if detail.synopsis.length > 360}
							<button
								type="button"
								class="prose-toggle"
								onclick={() => (synopsisExpanded = !synopsisExpanded)}
								aria-expanded={synopsisExpanded}
							>
								{synopsisExpanded
									? m.detail_synopsis_toggle_less()
									: m.detail_synopsis_toggle_more()}
							</button>
						{/if}
					{:else}
						<p class="prose-empty">{m.detail_synopsis_empty()}</p>
					{/if}
				</div>

				{#if !singleVideo}
					<!-- Single-video shows (movies, finished one-ep
					     OVAs/specials) collapse to "the show IS the video":
					     a one-tile Episodes section adds zero information
					     and just stretches the page. Hide the entire block;
					     the play CTA above already says "Watch". -->
					<div class="body-col body-col-episodes">
						<!-- Episodes toolbar — same layout as /play/[id] for cross-
					     page consistency: heading + range on the left, jump
					     pill + prev/next chevrons on the right. The
					     editorial scrubber experiment was removed in favour
					     of this simpler shape; both pages now read the
					     same. -->
						<div class="ep-toolbar">
							<div class="ep-toolbar-left">
								<h2 class="ep-section-heading">{m.detail_episodes_heading()}</h2>
								<span class="ep-range">
									{#if episodes && episodes.length > 0 && detail.episode_count}
										{#if knownAvailableEpisodes !== null && knownAvailableEpisodes < detail.episode_count}
											<!-- Currently-airing shows: phrase the gap as
										     "{N} aired so far · {M} expected" so the count is
										     read as a release-progress indicator. Other gaps
										     (Kitsu missing data on a finished show) read as
										     "{N} of {M} listed". -->
											{#if detail.status === 'current'}
												{m.detail_episodes_range_aired_so_far({
													count: String(knownAvailableEpisodes)
												})}
												<span class="ep-range-faint"
													>{m.detail_episodes_range_expected_suffix({
														count: String(detail.episode_count)
													})}</span
												>
											{:else if detail.status === 'upcoming'}
												{m.detail_episodes_range_aired({ count: String(knownAvailableEpisodes) })}
												<span class="ep-range-faint"
													>{m.detail_episodes_range_announced_suffix({
														count: String(detail.episode_count)
													})}</span
												>
											{:else}
												{m.detail_episodes_range_of({
													aired: String(knownAvailableEpisodes),
													total: String(detail.episode_count)
												})}
												<span class="ep-range-faint">{m.detail_episodes_range_listed_suffix()}</span
												>
											{/if}
										{:else if totalEpisodePages !== null && totalEpisodePages > 1}
											{m.detail_episodes_range_pagination({
												epStart: String(epStart),
												epEnd: String(epEnd),
												total: String(detail.episode_count)
											})}
										{:else}
											{m.detail_episodes_range_all({ count: String(detail.episode_count) })}
										{/if}
									{:else if episodes && episodes.length > 0}
										{m.detail_episodes_range_page({ page: String(episodesPage) })}
									{:else if episodesError}
										{m.detail_episodes_range_unavailable()}
									{:else}
										{m.detail_episodes_range_loading()}
									{/if}
								</span>
							</div>

							{#if (totalEpisodePages !== null && totalEpisodePages > 1) || (episodes && pageWhole === UI_PAGE_SIZE)}
								<div class="ep-toolbar-right">
									<form class="ep-jump" onsubmit={jumpToEpisode}>
										<span class="ep-jump-key" aria-hidden="true"
											>{m.detail_episodes_jump_label()}</span
										>
										<span class="ep-jump-pill">
											<input
												class="jump-input"
												type="number"
												min="1"
												max={detail.episode_count ?? 9999}
												step="1"
												placeholder={m.detail_episodes_jump_placeholder()}
												aria-label={m.detail_episodes_jump_placeholder()}
												bind:value={jumpInput}
											/>
											<button
												type="submit"
												class="ep-jump-go"
												disabled={!jumpInput || episodesLoading}
												aria-label={m.detail_episodes_jump_submit_aria_label()}
											>
												<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
													<path
														d="M9 5l7 7-7 7"
														fill="none"
														stroke="currentColor"
														stroke-width="2.5"
														stroke-linecap="round"
														stroke-linejoin="round"
													/>
												</svg>
											</button>
										</span>
									</form>
									<div
										class="ep-pager-mini"
										role="group"
										aria-label={m.detail_episodes_pager_aria_label()}
									>
										<button
											type="button"
											class="ep-pager-mini-btn"
											onclick={() => gotoPage(episodesPage - 1)}
											disabled={episodesPage <= 1 || episodesLoading}
											aria-label={m.detail_episodes_pager_previous_aria_label()}
										>
											<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
												<path
													d="M15 5l-7 7 7 7"
													fill="none"
													stroke="currentColor"
													stroke-width="2.5"
													stroke-linecap="round"
													stroke-linejoin="round"
												/>
											</svg>
										</button>
										<button
											type="button"
											class="ep-pager-mini-btn"
											onclick={() => gotoPage(episodesPage + 1)}
											disabled={(totalEpisodePages !== null && episodesPage >= totalEpisodePages) ||
												episodesLoading ||
												(episodes !== null && pageWhole < UI_PAGE_SIZE)}
											aria-label={m.detail_episodes_pager_next_aria_label()}
										>
											<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
												<path
													d="M9 5l7 7-7 7"
													fill="none"
													stroke="currentColor"
													stroke-width="2.5"
													stroke-linecap="round"
													stroke-linejoin="round"
												/>
											</svg>
										</button>
									</div>
								</div>
							{/if}
						</div>
						<!--
				  No {#key episodesPage} wrapping the <ul>: that destroyed
				  the parent on every page change, taking the children with
				  it before their out: transitions could run, which is why
				  the previous build looked like an instant swap. Now the
				  <ul> stays mounted; the keyed each block (key=ep.id) does
				  the diff. Old LIs run out:settleOut, new LIs run
				  in:settle, with a stagger via per-index delay so episodes
				  land left-to-right, top-to-bottom.
				-->
						<ul class="ep-grid">
							{#if episodes === null}
								<!-- Skeleton while fetch is in flight -->
								{#each Array.from({ length: 6 }, (_, k) => k) as i (i)}
									<li>
										<div class="ep-tile ep-tile-skel" aria-hidden="true">
											<div class="ep-thumb ep-thumb-skel"></div>
											<div class="ep-foot-skel"></div>
										</div>
									</li>
								{/each}
							{:else if episodes.length > 0}
								<!-- Real Kitsu data path; per-tile staggered enter for a
						     premium feel — tiles flow in left-to-right, top-to-bottom. -->
								{#each episodes as ep, i (ep.id)}
									{@const thumb = imageProxyUrl(ep.thumbnail?.original ?? null)}
									{@const num = ep.number ?? ep.relative_number ?? null}
									{@const air =
										num !== null ? epAirState(num, airing) : { unaired: false as const }}
									{@const capGated =
										!air.unaired && num !== null && beyondPlayable(num, playableEpisodeCount)}
									<li
										class:ep-highlight={num !== null && num === highlightEp}
										data-ep-num={num ?? ''}
										in:settle={{ duration: 620, delay: i * 45 }}
										out:settleOut={{ duration: 320, delay: i * 18 }}
									>
										<button
											type="button"
											class="ep-tile"
											class:ep-tile-disabled={availability === false}
											class:ep-tile-recheck={capGated && availability !== false}
											class:ep-tile-unaired={air.unaired}
											aria-disabled={availability === false || airingIsPending}
											title={air.unaired
												? m.detail_ep_unaired_recheck_tooltip()
												: availability === false
													? m.detail_ep_disabled_tooltip()
													: capGated
														? m.detail_ep_recheck_idle()
														: undefined}
											onclick={() => {
												// `aria-disabled` is advisory — it does not stop a click, and a
												// delisting leaves the tile cap-gated because it arrives without
												// a count. Without this the re-ask branch below would ask
												// the provider again about a show it just said it does not have.
												if (availability === false) return;
												if (airingIsPending) return;
												// Unaired is the schedule's word, and cached: ask again.
												if (air.unaired) return void onUnairedEpisode(num ?? 0);
												// Cap-gated is not "disabled": the count is a
												// mount-time snapshot, so re-ask before refusing.
												if (capGated) onRecheckEpisode(num ?? 0);
												else onPickEpisode(num ?? 0);
											}}
										>
											<span class="ep-thumb">
												{#if thumb}
													<img src={thumb} alt="" loading="lazy" decoding="async" />
												{:else if air.unaired}
													<span class="ep-thumb-placeholder ep-thumb-airs" aria-hidden="true">
														{unairedLabel(air.airsAt)}
													</span>
												{:else}
													<span class="ep-thumb-placeholder" aria-hidden="true">
														{num ? num.toString().padStart(2, '0') : '·'}
													</span>
												{/if}
												<span class="ep-frame-num" aria-hidden="true">
													<span class="ep-frame-num-key">{m.detail_episode_frame_num_key()}</span>
													<span class="ep-frame-num-val">{num ?? '?'}</span>
												</span>
											</span>
											<span class="ep-foot">
												<span class="ep-title">
													{ep.canonical_title ??
														fallbackTileTitle(
															num,
															(n) => m.detail_episode_title_fallback({ num: n }),
															(n) => m.detail_episode_extra_title_fallback({ num: n })
														)}
												</span>
												<span class="ep-meta">
													{#if air.unaired && thumb}
														<span class="ep-unaired-label">{unairedLabel(air.airsAt)}</span>
													{:else if !air.unaired && ep.length}<span
															>{m.detail_episode_length_suffix({
																minutes: String(ep.length)
															})}</span
														>{/if}
												</span>
											</span>
										</button>
									</li>
								{/each}
							{:else if showEpPlaceholders}
								<!-- Fallback: Kitsu didn't have episode data, but episode_count
						     gives us a usable count. Render numbered placeholder tiles
						     so the user isn't blocked from poking the panel. -->
								{#each Array.from({ length: epPlaceholderCount }, (_, k) => k + 1) as n, i (n)}
									{@const air = epAirState(n, airing)}
									{@const capGated = !air.unaired && beyondPlayable(n, playableEpisodeCount)}
									<li
										class:ep-highlight={n === highlightEp}
										data-ep-num={n}
										in:settle={{ duration: 580, delay: i * 40 }}
										out:settleOut={{ duration: 300, delay: i * 16 }}
									>
										<button
											type="button"
											class="ep-tile"
											class:ep-tile-disabled={availability === false}
											class:ep-tile-recheck={capGated && availability !== false}
											class:ep-tile-unaired={air.unaired}
											aria-disabled={availability === false || airingIsPending}
											title={air.unaired
												? m.detail_ep_unaired_recheck_tooltip()
												: availability === false
													? m.detail_ep_disabled_tooltip()
													: capGated
														? m.detail_ep_recheck_idle()
														: undefined}
											onclick={() => {
												// `aria-disabled` is advisory — it does not stop a click, and a
												// delisting leaves the tile cap-gated because it arrives without
												// a count. Without this the re-ask branch below would ask
												// the provider again about a show it just said it does not have.
												if (availability === false) return;
												if (airingIsPending) return;
												if (air.unaired) return void onUnairedEpisode(n);
												if (capGated) onRecheckEpisode(n);
												else onPickEpisode(n);
											}}
										>
											<span class="ep-thumb">
												{#if air.unaired}
													<span class="ep-thumb-placeholder ep-thumb-airs" aria-hidden="true">
														{unairedLabel(air.airsAt)}
													</span>
												{:else}
													<span class="ep-thumb-placeholder" aria-hidden="true">
														{n.toString().padStart(2, '0')}
													</span>
												{/if}
												<span class="ep-tag" aria-hidden="true">
													<span class="ep-tag-key">{m.detail_episode_frame_num_key()}</span>
													<span class="ep-tag-num">{n}</span>
												</span>
											</span>
											<span class="ep-foot">
												<span class="ep-title"
													>{m.detail_episode_placeholder_title({ num: String(n) })}</span
												>
												<span class="ep-meta">{m.detail_episode_placeholder_meta()}</span>
											</span>
										</button>
									</li>
								{/each}
							{/if}
						</ul>
						{#if episodesError}
							<p class="ep-grid-foot ep-grid-foot-warn">
								{m.detail_episodes_error_message()}
							</p>
						{:else if episodes && episodes.length > 0}
							<p class="ep-grid-foot">{m.detail_episodes_source_credit()}</p>
						{/if}
					</div>
				{/if}
			</section>

			<!-- Similar titles strip — placeholder for AniList recommendations.
		     Today: re-uses kitsuSearch with the canonical title's first
		     1-2 words to surface franchise neighbours / look-alikes. -->
			{#if similar && similar.length > 0}
				<section class="similar">
					<Strip
						eyebrow={m.detail_similar_strip_eyebrow()}
						caption={m.detail_similar_strip_caption()}
					>
						{#each similar as hit (hit.id)}
							<PosterCard anime={hit} />
						{/each}
					</Strip>
				</section>
			{/if}
		</div>
	{/if}
</main>

<LoadingOverlay visible={actionBusy || resumeWaiting} progress={actionProgress} />

{#if playFailure}
	<ErrorOverlay
		headline={singleVideo
			? m.detail_error_play_headline_single()
			: m.detail_error_play_headline({ episode: String(playFailure.episode) })}
		body={playFailure.message}
		onDismiss={() => (playFailure = null)}
	/>
{/if}

<DownloadConfirm
	bind:open={downloadModalOpen}
	args={downloadArgs}
	defaultDir={downloadDefaultDir}
	availableEpisodes={airedCap(minCap(knownAvailableEpisodes, playableEpisodeCount), airing)}
	showThisEpisode={false}
	onClose={() => (downloadModalOpen = false)}
/>

<style>
	.page {
		/* Route root spans the full main-area so the hero can paint
		   edge-to-edge on ultrawides. The capped reading column lives
		   on `.content` below. */
		padding-block-end: var(--space-8);
		/* Page-enter animation — a soft fade + lift so navigating into a
		   detail page doesn't feel like a hard cut. The reduced-motion
		   token already zeroes --dur-slow so this is inert when the user
		   opts out. */
		animation: detail-page-enter var(--dur-slow) var(--ease-out-soft) both;
	}

	/* Capped reading column for everything below the hero. The banner
	   stays cinematic (full main-area), while the content stays
	   deliberate at 90rem so titles, synopsis and episode tiles don't
	   sprawl across an ultrawide. */
	.content {
		max-inline-size: 90rem;
		margin-inline: auto;
	}
	@keyframes detail-page-enter {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	/* — Hero band. */
	.hero {
		position: relative;
		aspect-ratio: var(--hero-aspect);
		overflow: hidden;
		/* Escape the layout's inline gutters AND the rail column on
		   the left so the banner spreads behind the glassy rail too
		   (the rail's backdrop-blur frosts whatever shows through).
		   Right side just escapes the gutter — there's no chrome
		   there to bleed behind. */
		margin-inline-start: calc(-1 * (var(--space-7) + var(--rail-width)));
		margin-inline-end: calc(-1 * var(--space-7));
		inline-size: calc(100% + 2 * var(--space-7) + var(--rail-width));
		/* Bleed under the glassy topbar: pull the banner up by the
		   topbar's block size so it spreads behind the chrome and
		   the topbar's backdrop-blur frosts the visible portion.
		   margin-block-end below compensates so the masthead under
		   the hero stays at its original y. */
		margin-block-start: calc(-1 * var(--topbar-h));
		/* Per-anime accent gradient shows through when Kitsu has no
		   cover, no poster, and no episode thumbnail — the show
		   still gets a branded backdrop instead of dead grey. The
		   <img> below paints over this when available. */
		background:
			radial-gradient(
				ellipse 80% 60% at 30% 30%,
				color-mix(in oklab, var(--accent) 35%, var(--ink-050)) 0%,
				color-mix(in oklab, var(--accent) 12%, var(--ink-050)) 45%,
				var(--ink-050) 100%
			),
			var(--ink-050);
		/* Compensate for the negative margin-block-start above so the
		   masthead beneath the hero stays at its original y. */
		margin-block-end: calc(var(--space-7) + var(--topbar-h));
		/* Bleed the banner image down into the masthead area: mask
		   the bottom 30% of the hero to transparent so the image
		   fades over the page background. The masthead then pulls
		   up further (margin-block-start below) so its content
		   overlaps the faded zone — banner appears to continue
		   behind the metadata, blurring out as it goes. */
		mask-image: linear-gradient(180deg, #000 0%, #000 70%, transparent 100%);
		-webkit-mask-image: linear-gradient(180deg, #000 0%, #000 70%, transparent 100%);
		/* Hero scales up subtly on entry — feels like the cover comes
		   forward to the screen, per user's M3.7-era request. */
		animation: detail-hero-enter var(--dur-slow) var(--ease-out-soft) both;
	}
	@keyframes detail-hero-enter {
		from {
			transform: scale(1.04);
			filter: brightness(0.7);
		}
		to {
			transform: scale(1);
			filter: brightness(1);
		}
	}
	.hero-img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		object-position: center 30%;
		will-change: transform;
		animation: hero-in var(--dur-slow) var(--ease-out-soft) both;
	}
	@keyframes hero-in {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}
	.hero-fallback .hero-img {
		filter: blur(28px) brightness(0.55) saturate(0.85);
	}
	.hero-scrim {
		position: absolute;
		inset: 0;
		background:
			linear-gradient(
				180deg,
				color-mix(in oklab, var(--ink-000) 25%, transparent) 0%,
				color-mix(in oklab, var(--ink-000) 0%, transparent) 35%,
				color-mix(in oklab, var(--ink-000) 60%, transparent) 75%,
				var(--ink-000) 100%
			),
			linear-gradient(
				90deg,
				color-mix(in oklab, var(--ink-000) 60%, transparent) 0%,
				color-mix(in oklab, var(--ink-000) 0%, transparent) 35%
			);
		pointer-events: none;
	}
	/* Under the framings beyond the top crop ($lib/hero/framing) the
	   band's fade shortens to the strip the masthead's text sits on,
	   and the darkening keeps to that strip plus a thin band under the
	   topbar. */
	.hero.framing-contain,
	.hero.framing-pillar,
	.hero.framing-focus {
		mask-image: linear-gradient(180deg, #000 0%, #000 84%, transparent 100%);
		-webkit-mask-image: linear-gradient(180deg, #000 0%, #000 84%, transparent 100%);
		container-type: size;
	}
	.framing-contain .hero-scrim,
	.framing-pillar .hero-scrim,
	.framing-focus .hero-scrim {
		background:
			linear-gradient(
				180deg,
				color-mix(in oklab, var(--ink-000) 45%, transparent) 0%,
				transparent calc(var(--topbar-h) * 2)
			),
			linear-gradient(
				180deg,
				transparent 70%,
				color-mix(in oklab, var(--ink-000) 55%, transparent) 100%
			);
	}
	.hero.framing-focus:not(.hero-fallback) .hero-img {
		object-position: 50% 50%;
	}
	.hero-backdrop {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		filter: blur(44px) brightness(0.28) saturate(1.1);
		animation: hero-in var(--dur-slow) var(--ease-out-soft) both;
	}
	/* Fill the height: the whole upload at the band's full height,
	   centred — pillarboxed over its backdrop when narrower than the
	   band, cropped at the sides only when wider. */
	.hero.framing-pillar:not(.hero-fallback) .hero-img {
		--ratio: 4.2;
		inset-block: 0;
		inset-inline-start: calc(50% - 100cqb * var(--ratio) / 2);
		block-size: 100cqb;
		inline-size: calc(100cqb * var(--ratio));
		max-inline-size: none;
		object-fit: cover;
		mask-image: linear-gradient(90deg, transparent, #000 4%, #000 96%, transparent);
	}
	.hero.framing-contain:not(.hero-fallback) .hero-img {
		inset: 0;
		margin: auto;
		/* Until the image decodes and its own ratio arrives inline. */
		--ratio: 4.2;
		inline-size: min(100cqi, 100cqb * var(--ratio));
		block-size: auto;
		aspect-ratio: auto var(--ratio);
		object-fit: cover;
		mask-image:
			linear-gradient(90deg, transparent, #000 4%, #000 96%, transparent),
			linear-gradient(180deg, transparent, #000 12%, #000 88%, transparent);
		mask-composite: intersect;
	}
	.hero-grain {
		position: absolute;
		inset: 0;
		opacity: 0.18;
		pointer-events: none;
		background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='160' height='160'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 0.6 0'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>");
		background-size: 160px 160px;
		mix-blend-mode: overlay;
	}

	/* — Masthead: poster hangs into hero. */
	.masthead {
		display: grid;
		grid-template-columns: minmax(12rem, 16rem) 1fr;
		gap: var(--space-7);
		padding-inline: var(--space-6);
		/* Pull deep into the hero's faded zone — the bottom ~30% of
		   the banner fades via mask-image, and the masthead text
		   should sit inside that fade so the banner reads as
		   continuing behind the metadata, not just stopping above
		   it. ~10rem matches the fade region at typical viewports. */
		margin-block-start: -10rem;
		align-items: end;
		position: relative;
	}
	@media (max-inline-size: 720px) {
		.masthead {
			grid-template-columns: 1fr;
			margin-block-start: calc(-1 * var(--space-8));
		}
	}
	.poster-frame {
		position: relative;
		aspect-ratio: var(--poster-aspect);
		background: var(--ink-100);
		border-radius: var(--radius-card);
		overflow: hidden;
		box-shadow:
			0 1px 0 1px color-mix(in oklab, var(--accent) 40%, transparent),
			var(--shadow-card-hover);
		transform: translateY(calc(-1 * var(--space-6)));
		animation: poster-in var(--dur-slow) var(--ease-out-elastic) both;
	}
	@keyframes poster-in {
		from {
			opacity: 0;
			transform: translateY(0) scale(0.96);
		}
		to {
			opacity: 1;
			transform: translateY(calc(-1 * var(--space-6))) scale(1);
		}
	}
	.poster-img {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
	}
	.poster-placeholder {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		padding: var(--space-4);
		background: linear-gradient(180deg, var(--ink-100), var(--ink-050));
	}
	.poster-placeholder-title {
		font-family: var(--font-body);
		font-weight: 600;
		font-size: var(--type-display-m);
		color: var(--bone-200);
		text-align: center;
	}

	.masthead-text {
		padding-block-end: var(--space-2);
	}

	.eyebrow {
		margin: 0 0 var(--space-3);
	}
	.eyebrow-rule {
		/* masthead eyebrow rides the per-show accent for theming punch. */
		background: var(--accent);
	}

	.title {
		margin: 0 0 var(--space-5);
		font-family: var(--font-display);
		font-size: clamp(2rem, 4.4vw, var(--type-display-xl));
		line-height: var(--leading-tight);
		letter-spacing: var(--tracking-display);
		color: var(--bone-100);
		font-style: italic;
		animation: text-in var(--dur-med) var(--ease-out-soft) both;
		animation-delay: 60ms;
	}
	@keyframes text-in {
		from {
			opacity: 0;
			transform: translateY(6px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	/* — Action row. */
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3);
		margin-block-end: var(--space-5);
	}
	.actions-loading {
		min-block-size: 2.75rem;
	}
	.action-skel {
		display: inline-block;
		inline-size: 11rem;
		block-size: 2.5rem;
		border-radius: var(--radius-pill);
		background: linear-gradient(
			90deg,
			color-mix(in oklab, var(--ink-200) 40%, transparent),
			color-mix(in oklab, var(--ink-300) 50%, transparent),
			color-mix(in oklab, var(--ink-200) 40%, transparent)
		);
		background-size: 200% 100%;
		animation: action-skel-pulse 1.4s ease-in-out infinite;
	}
	.action-skel-narrow {
		inline-size: 7rem;
	}
	@keyframes action-skel-pulse {
		0% {
			background-position: 0% 50%;
		}
		100% {
			background-position: -200% 50%;
		}
	}
	.unavailable {
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		margin: 0 0 var(--space-5);
		padding: var(--space-3) var(--space-4);
		background: color-mix(in oklab, var(--ink-100) 60%, transparent);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-card);
		font-family: var(--font-body);
		font-size: var(--type-meta);
		color: var(--bone-200);
	}
	.unavailable > [aria-hidden] {
		color: var(--bone-400);
		font-family: var(--font-mono);
	}
	.btn {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-5);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-100);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-control);
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-fast) var(--ease-out-soft),
			transform var(--dur-fast) var(--ease-out-soft);
	}
	.btn:hover {
		transform: translateY(-1px);
	}
	/* btn-primary is no longer used here — Play episode 1 uses
	   .btn-glass with --btn-glow=var(--accent). Removed to satisfy the
	   unused-selector check; restore if a future button needs the flat
	   accent background. */
	.btn-outline {
		color: var(--bone-100);
		border-color: var(--bone-300);
	}
	.btn-outline:hover {
		border-color: var(--bone-100);
	}
	/* — Segmented controls (sub/dub + quality). */
	.controls {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-5);
		align-items: center;
		margin-block-end: var(--space-5);
	}
	.seg-group {
		display: inline-flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	.seg-label {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.seg {
		display: inline-flex;
		border: 1px solid var(--ink-300);
		border-radius: 2px;
		overflow: hidden;
	}
	.seg-btn {
		padding: var(--space-2) var(--space-4);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
		background: transparent;
		border: 0;
		border-inline-end: 1px solid var(--ink-300);
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.seg-btn:last-child {
		border-inline-end: 0;
	}
	.seg-btn:hover:not(:disabled):not(.active) {
		color: var(--bone-100);
	}
	.seg-btn.active {
		background: var(--accent);
		color: var(--ink-000);
	}
	.seg-btn:disabled {
		opacity: 0.5;
		cursor: progress;
	}
	.seg-narrow .seg-btn {
		padding-inline: var(--space-3);
	}
	.seg-error {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		color: var(--accent-oxblood);
	}

	/* — Meta row: stat strip with fixed min-width per pill so
	   "Year / Episodes / Rating / Age / Rank" lay out as a tabular
	   row, not a stretchy flex line. Separators are 1px accent-
	   tinted rules — more intentional than the previous near-
	   invisible ink-200 hairline. */
	.meta-row {
		margin: 0;
		padding: var(--space-4) 0 0;
		list-style: none;
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-4) 0; /* row gap only — column spacing comes from per-pill padding */
		border-block-start: 1px solid var(--accent);
	}
	.meta-pill {
		display: inline-flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-1);
		flex: 0 0 auto;
		min-inline-size: 7rem;
		padding-inline: var(--space-5);
		text-align: center;
		/* Each pill carries a leading separator. The first pill in the
		   strip drops it (selector below). When the strip wraps, the
		   first pill of subsequent rows keeps its leading rule, which
		   acts as a row-start anchor — visually fine for an editorial
		   stat strip. */
		border-inline-start: 1px solid color-mix(in oklab, var(--accent) 35%, var(--ink-200));
	}
	.meta-pill:first-child {
		padding-inline-start: 0;
		border-inline-start: 0;
	}
	.meta-key {
		font-family: var(--font-body);
		font-size: 0.75rem; /* 12px */
		font-weight: 600;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.meta-val {
		font-family: var(--font-body);
		font-size: var(--type-body-l);
		font-weight: 500;
		color: var(--bone-100);
	}
	.meta-val.num {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		letter-spacing: 0;
	}
	.meta-faint {
		color: var(--bone-300);
		font-size: var(--type-meta);
		margin-inline-start: 2px;
	}
	.star {
		color: var(--accent);
		margin-inline-end: 4px;
	}

	/* — Action notice (inline status banner under masthead). */
	.action-notice {
		margin: var(--space-6) var(--space-6) 0;
		padding: var(--space-3) var(--space-4);
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
		font-family: var(--font-body);
		font-size: var(--type-meta);
		color: var(--bone-100);
		background: color-mix(in oklab, var(--accent) 6%, var(--ink-050));
		border-inline-start: 2px solid var(--accent);
		animation: text-in var(--dur-med) var(--ease-out-soft) both;
	}
	/* "NOTE" stays a small uppercase badge — that's a label, not a sentence. */
	.action-notice-key {
		font-family: var(--font-body);
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.action-notice-rule {
		inline-size: 1.5rem;
		block-size: 1px;
		background: color-mix(in oklab, var(--bone-100) 30%, transparent);
	}

	/* — Body: vertical stack of synopsis → episodes. The earlier 2-col
	     layout produced an imbalance whenever one block was much taller
	     than the other. Stacked, both panels span the full editorial
	     column width and never compete for vertical alignment.
	     Container is the wide cap (matches the hero above), so widescreens
	     don't leave huge empty side margins. The synopsis used to cap
	     at 70ch for prose readability, but that left a ragged disjoint
	     against the much-wider episode grid below. Now both blocks share
	     the full body width — synopsis is short enough that the wider
	     line length doesn't hurt comprehension, and the collapsed
	     preview shows more text in the same vertical space. */
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-8);
		max-inline-size: var(--content-max-wide);
		margin: var(--space-7) auto 0;
		padding-inline: var(--space-8);
	}

	.body-col-prose {
		inline-size: 100%;
	}
	.section-eyebrow {
		margin: 0 0 var(--space-4);
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-200);
		font-weight: 600;
	}
	/* Synopsis — collapsed by default to a 5-line preview with a soft
	   gradient fade at the bottom; expands on user click. The font is
	   bumped a notch (display-m, was body-l) so the prose feels like a
	   proper editorial spread instead of body chrome. */
	.prose-wrap {
		position: relative;
		max-block-size: 9.5rem;
		overflow: hidden;
		transition: max-block-size var(--dur-slow) var(--ease-out-soft);
	}
	.prose-wrap.expanded {
		max-block-size: 200rem;
	}
	.prose {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-body-l);
		line-height: 1.55;
		color: color-mix(in oklab, var(--bone-100) 85%, transparent);
	}
	.prose-fade {
		position: absolute;
		inset-block-end: 0;
		inset-inline: 0;
		block-size: 4rem;
		background: linear-gradient(180deg, transparent 0%, var(--ink-000) 90%);
		pointer-events: none;
		transition: opacity var(--dur-fast) var(--ease-out-soft);
	}
	.prose-wrap.expanded .prose-fade {
		opacity: 0;
	}
	.prose-toggle {
		margin-block-start: var(--space-3);
		padding: 4px 0;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-200);
		border-block-end: 1px solid var(--bone-300);
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.prose-toggle:hover {
		color: var(--bone-100);
		border-block-end-color: var(--bone-100);
	}
	.prose-empty {
		margin: 0;
		font-family: var(--font-body);
		color: var(--bone-300);
	}

	/* — Episodes panel. Auto-fill grid that adapts to the viewport:
	     ~3-4 columns at standard widths, 4-5 at widescreen. Tiles always
	     ≥ 18rem wide so thumbnails stay readable and titles fit. */
	.body-col-episodes {
		inline-size: 100%;
	}
	.ep-grid {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
		gap: var(--space-4);
	}
	/* Tile is now thumbnail-led: 16:9 image at top, mono-numeral overlay
	   tag in the corner, title + duration in the foot. Forward-compatible
	   with future episodes with no thumbnail (placeholder gradient
	   showing the episode number). */
	.ep-tile {
		inline-size: 100%;
		display: grid;
		grid-template-rows: auto 1fr;
		gap: 0;
		padding: 0;
		text-align: start;
		background: var(--ink-050);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-card);
		overflow: hidden;
		/* Origin sits low so scale-up on hover lifts upward toward the
		   poster + thumbnail rather than pushing into the next row. */
		transform-origin: 50% 80%;
		opacity: 1;
		/* Default opacity transition is the SLOW return — when the
		   highlight class is removed from the parent grid, sibling
		   tiles fade back to full over 1.4s, so the spotlight ends
		   with a graceful unwind, not a hard cut. The fast dim-in
		   timing is set on the active rule below. */
		transition:
			transform var(--dur-med) var(--ease-out-elastic),
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-med) var(--ease-out-soft),
			opacity 1.4s var(--ease-out-soft);
	}
	.ep-tile:hover:not(.ep-tile-disabled) {
		/* More expressed pop: lift, scale, accent-tinted shadow halo. */
		transform: translateY(-4px) scale(1.04);
		z-index: 1;
		border-color: color-mix(in oklab, var(--accent) 80%, var(--ink-300));
		box-shadow:
			0 12px 28px -6px color-mix(in oklab, var(--accent) 28%, transparent),
			0 4px 10px -4px rgb(0 0 0 / 0.45);
	}

	.ep-tile:hover:not(.ep-tile-disabled) .ep-thumb img {
		filter: brightness(1);
	}

	/* Availability gate — when the provider doesn't index the show, the
	   tiles still render (Kitsu metadata) but as read-only thumbs. */
	.ep-tile-disabled {
		cursor: not-allowed;
		opacity: 0.55;
		filter: saturate(0.6);
	}

	/* Aired, but above the episode count the provider last reported. Dimmed
	   the same amount as an uncatalogued tile — the episode genuinely
	   is not streamable yet — but it keeps the pointer and the hover
	   lift, because clicking it re-asks the provider. `not-allowed` here
	   would deny the one affordance the tile exists to offer. */
	.ep-tile-recheck {
		cursor: pointer;
		opacity: 0.55;
		filter: saturate(0.6);
	}

	/* Unaired episodes: visible so the season's shape stays readable,
	   and dimmer than a recheck tile — the schedule says the episode is
	   not out. But the schedule is cached, so a click checks it again:
	   the tile keeps the pointer and the hover lift, like
	   `.ep-tile-recheck`. */
	.ep-tile-unaired {
		cursor: pointer;
		opacity: 0.45;
		filter: saturate(0.35);
	}
	.ep-unaired-label {
		font-variant-caps: all-small-caps;
		letter-spacing: 0.04em;
	}
	/* Center-of-tile variant: the airs info takes the number's place
	   (the episode number stays visible in the corner tag). Sized to
	   read at a glance without shouting like the display-size digits. */
	.ep-thumb-airs {
		font-family: inherit;
		font-size: 1.05rem;
		font-weight: 600;
		letter-spacing: 0.03em;
		line-height: 1.3;
		text-align: center;
		padding-inline: 0.75rem;
		text-wrap: balance;
	}

	/* Spotlight: while the grid contains a highlighted tile, every
	   non-highlighted tile dims to 0.35. Dim-in is fast (0.4s) so the
	   spotlight engages decisively; the dim-out is the longer 1.4s
	   transition on `.ep-tile` itself, so when the highlight class
	   clears the surrounding tiles fade back up gradually instead of
	   snapping. */
	.ep-grid:has(li.ep-highlight) li:not(.ep-highlight) .ep-tile {
		opacity: 0.35;
		transition: opacity 0.4s var(--ease-out-soft);
	}

	/* Jump highlight: when the user submits the Jump field, the matching
	   tile pulses an accent ring twice and stays ringed for ~3.2s so the
	   user sees "this is the one you asked for." Class is auto-removed
	   by the script. Mirrors the prior Continue Watching deep-link cue. */
	.ep-grid li.ep-highlight .ep-tile {
		border-color: color-mix(in oklab, var(--accent) 90%, var(--bone-100));
		box-shadow:
			0 0 0 2px color-mix(in oklab, var(--accent) 80%, transparent),
			0 16px 32px -8px color-mix(in oklab, var(--accent) 38%, transparent);
		animation: ep-highlight-pulse 1.6s ease-out 2;
	}
	@keyframes ep-highlight-pulse {
		0% {
			box-shadow:
				0 0 0 0 color-mix(in oklab, var(--accent) 70%, transparent),
				0 0 0 0 color-mix(in oklab, var(--accent) 30%, transparent);
		}
		35% {
			box-shadow:
				0 0 0 4px color-mix(in oklab, var(--accent) 70%, transparent),
				0 0 24px 4px color-mix(in oklab, var(--accent) 35%, transparent);
		}
		100% {
			box-shadow:
				0 0 0 2px color-mix(in oklab, var(--accent) 70%, transparent),
				0 16px 32px -8px color-mix(in oklab, var(--accent) 30%, transparent);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.ep-grid li.ep-highlight .ep-tile {
			animation: none;
		}
		.ep-grid:has(li.ep-highlight) li:not(.ep-highlight) .ep-tile {
			opacity: 1;
		}
	}

	.ep-thumb {
		position: relative;
		display: block;
		aspect-ratio: 16 / 9;
		overflow: hidden;
		background: var(--ink-100);
	}
	/* Bottom scrim under the frame-number — keeps the oversized "EP 24"
	   readable on bright thumbnails without painting a chip. Pseudo-
	   element sits between the img and the frame-num via the .ep-thumb
	   layering. */
	.ep-thumb::after {
		content: '';
		position: absolute;
		inset-block-end: 0;
		inset-inline: 0;
		block-size: 55%;
		background: linear-gradient(
			180deg,
			transparent 0%,
			color-mix(in oklab, var(--ink-000) 75%, transparent) 100%
		);
		pointer-events: none;
		opacity: 0.85;
		transition: opacity var(--dur-fast) var(--ease-out-soft);
	}
	.ep-tile:hover .ep-thumb::after {
		opacity: 1;
	}
	.ep-thumb img {
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		filter: brightness(0.85);
		transition: filter var(--dur-med) var(--ease-out-soft);
	}
	.ep-thumb-placeholder {
		display: grid;
		place-items: center;
		inline-size: 100%;
		block-size: 100%;
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-display-m);
		color: var(--bone-300);
		background: linear-gradient(
			135deg,
			var(--ink-100),
			color-mix(in oklab, var(--accent) 18%, var(--ink-100))
		);
	}
	/* Episode frame-number — film-print style. The number lives in the
	   bottom-left of the thumb, oversized mono numeral, no chip
	   background. A subtle gradient scrim on the thumb keeps it legible
	   against any image. On hover the numeral lifts + tints accent —
	   reads as part of the card composition, not a tag glued on. */
	.ep-frame-num {
		position: absolute;
		inset-block-end: var(--space-3);
		inset-inline-start: var(--space-3);
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		line-height: 1;
		pointer-events: none;
		filter: drop-shadow(0 1px 2px rgb(0 0 0 / 0.65));
		transition:
			transform var(--dur-med) var(--ease-out-elastic),
			filter var(--dur-fast) var(--ease-out-soft);
		z-index: 1;
	}
	.ep-frame-num-key {
		font-family: var(--font-body);
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.18em;
		text-transform: uppercase;
		color: color-mix(in oklab, var(--bone-100) 75%, transparent);
		margin-block-end: 2px;
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	.ep-frame-num-val {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: 2.25rem; /* 36px */
		font-weight: 700;
		color: var(--bone-100);
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	.ep-tile:hover .ep-frame-num {
		transform: translateY(-4px);
		filter: drop-shadow(0 0 8px color-mix(in oklab, var(--accent) 50%, transparent));
	}
	.ep-tile:hover .ep-frame-num-val {
		color: var(--accent);
	}
	.ep-tile:hover .ep-frame-num-key {
		color: var(--bone-100);
	}
	.ep-foot {
		display: grid;
		gap: var(--space-1);
		padding: var(--space-3) var(--space-4);
		min-block-size: 5rem;
	}
	.ep-title {
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
		font-family: var(--font-body);
		font-size: 0.9375rem; /* 15px */
		font-weight: 500;
		line-height: 1.3;
		color: var(--bone-100);
	}
	.ep-meta {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-meta);
		color: var(--bone-400);
	}

	/* Skeleton + empty states for the episodes panel. */
	.ep-tile-skel {
		cursor: default;
	}
	.ep-thumb-skel {
		aspect-ratio: 16 / 9;
		background: var(--ink-100);
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	.ep-foot-skel {
		block-size: 4rem;
		background: var(--ink-050);
	}
	@media (prefers-reduced-motion: reduce) {
		.ep-thumb-skel {
			animation: none;
		}
	}

	/* Caption beneath the grid — sentence chrome (body sans, normal case)
	   so it reads as a footnote, not a tiny mono uppercase eyebrow. The
	   earlier styling buried the warning state in particular. */
	.ep-grid-foot {
		margin-block-start: var(--space-4);
		margin-block-end: 0;
		font-family: var(--font-body);
		font-size: var(--type-meta);
		font-weight: 500;
		color: var(--bone-300);
	}
	.ep-grid-foot-warn {
		color: var(--accent-oxblood);
		font-weight: 500;
	}

	/* Episode panel controls: jump-to-N input + prev/next pager. Sits
	   between the section header and the grid. Replaces the additive
	   load-more pattern (which made 500-episode shows render a vertical
	   wall on the page). */
	/* Single horizontal toolbar — jump on the left, range + pager on the
	   right. Avoids the previous "debug UI" feel of three rows of
	   scattered controls. Below 600px it stacks so nothing crushes. */
	.ep-toolbar {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--space-4);
		margin-block-end: var(--space-5);
	}
	@media (max-inline-size: 600px) {
		.ep-toolbar {
			flex-direction: column;
			align-items: stretch;
		}
	}
	.ep-toolbar-left,
	.ep-toolbar-right {
		display: flex;
		align-items: center;
		gap: var(--space-3);
	}
	.ep-toolbar-right {
		gap: var(--space-4);
	}

	/* Jump control — same vocabulary as the page-state display:
	   uppercase eyebrow label outside, marquee-style numerals inside
	   the pill, accent halo on focus, embedded chevron submit. */
	.ep-jump {
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
	}
	.ep-jump-key {
		font-family: var(--font-body);
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--bone-300);
		transition: color var(--dur-fast) var(--ease-out-soft);
	}
	.ep-jump:focus-within .ep-jump-key {
		color: var(--bone-200);
	}
	.ep-jump-pill {
		display: inline-flex;
		align-items: center;
		inline-size: 8.5rem;
		block-size: 2rem;
		padding-inline-start: var(--space-3);
		padding-inline-end: 4px;
		border-radius: var(--radius-pill);
		background: color-mix(in oklab, var(--bone-100) 7%, transparent);
		border: 1px solid color-mix(in oklab, var(--bone-100) 28%, transparent);
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-med) var(--ease-out-soft);
	}
	.ep-jump-pill:focus-within {
		border-color: var(--accent);
		background: color-mix(in oklab, var(--bone-100) 10%, transparent);
		box-shadow:
			0 0 8px color-mix(in oklab, var(--accent) 50%, transparent),
			0 0 16px color-mix(in oklab, var(--accent) 25%, transparent);
	}
	.jump-input {
		flex: 1;
		min-inline-size: 0; /* let the input shrink inside the pill */
		padding: 0;
		background: transparent;
		border: 0;
		outline: 0;
		color: var(--bone-100);
		/* Numerals match the page-state marquee voice — bold mono so
		   typed input feels like data on a projector display. */
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-body);
		font-weight: 600;
	}
	.jump-input::placeholder {
		font-family: var(--font-body);
		font-weight: 400;
		font-size: var(--type-meta);
		letter-spacing: 0;
		color: color-mix(in oklab, var(--bone-100) 55%, transparent);
	}
	.jump-input::-webkit-inner-spin-button,
	.jump-input::-webkit-outer-spin-button {
		appearance: none;
	}
	/* The pill's `:focus-within` rule already paints the accent halo,
	   so suppress the global `:focus-visible` ring on the bare input
	   — otherwise it shows through as a duplicate inner highlight. */
	.jump-input:focus,
	.jump-input:focus-visible {
		outline: 0;
		box-shadow: none;
	}
	/* Submit chevron embedded inside the pill, right edge. Reads as
	   "press to go" — solves the "where does Enter take me" problem
	   without an extra control taking up space. */
	.ep-jump-go {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 24px;
		block-size: 24px;
		padding: 0;
		border: 0;
		border-radius: var(--radius-pill);
		background: transparent;
		color: var(--bone-300);
		cursor: pointer;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			transform var(--dur-med) var(--ease-out-elastic);
	}
	.ep-jump-go:hover:not(:disabled),
	.ep-jump-go:focus-visible {
		color: var(--accent);
		background: color-mix(in oklab, var(--accent) 14%, transparent);
		transform: translateX(2px);
		outline: 0;
	}
	.ep-jump-go:disabled {
		color: color-mix(in oklab, var(--bone-100) 24%, transparent);
		cursor: not-allowed;
	}
	/* When the input has a value, brighten the chevron so the user
	   knows pressing Enter (or clicking) will do something. The
	   :has() selector pulls "filled" state across siblings without
	   needing a JS-driven class. */
	.ep-jump-pill:has(.jump-input:not(:placeholder-shown)) .ep-jump-go:not(:disabled) {
		color: var(--accent);
	}

	/* Toolbar pieces — heading + range on the left, paginator on the
	   right. Same shape as /play/[id]'s ep-toolbar so the two pages
	   read as the same control vocabulary. */
	.ep-section-heading {
		margin: 0;
		font-family: var(--font-body);
		font-size: 1.25rem; /* 20px — section, not display */
		font-weight: 600;
		line-height: 1.1;
		color: var(--bone-100);
		letter-spacing: -0.01em;
	}
	.ep-range {
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums lining-nums;
		font-size: var(--type-meta);
		font-weight: 500;
		letter-spacing: var(--tracking-meta);
		color: var(--bone-200);
	}
	.ep-range-faint {
		color: color-mix(in oklab, var(--bone-300) 70%, transparent);
	}

	/* Prev/next chevron mini-buttons — small bordered chips with
	   weighted SVG arrows. Hover lifts to accent + halo + chevron
	   slides in its pointing direction. */
	.ep-pager-mini {
		display: inline-flex;
		gap: var(--space-1);
	}
	.ep-pager-mini-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 2rem;
		block-size: 2rem;
		padding: 0;
		border: 1px solid color-mix(in oklab, var(--bone-100) 22%, transparent);
		border-radius: var(--radius-control);
		background: transparent;
		color: var(--bone-100);
		cursor: pointer;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			box-shadow var(--dur-fast) var(--ease-out-soft);
	}
	.ep-pager-mini-btn svg {
		transition: transform var(--dur-med) var(--ease-out-elastic);
	}
	.ep-pager-mini-btn:hover:not(:disabled) {
		color: var(--accent);
		border-color: color-mix(in oklab, var(--accent) 80%, transparent);
		background: color-mix(in oklab, var(--accent) 10%, transparent);
		box-shadow: 0 0 12px color-mix(in oklab, var(--accent) 30%, transparent);
	}
	.ep-pager-mini-btn:first-child:hover:not(:disabled) svg {
		transform: translateX(-2px);
	}
	.ep-pager-mini-btn:last-child:hover:not(:disabled) svg {
		transform: translateX(2px);
	}
	.ep-pager-mini-btn:disabled {
		color: var(--bone-400);
		border-color: color-mix(in oklab, var(--bone-100) 8%, transparent);
		cursor: not-allowed;
	}

	/* — Similar titles strip below the body. Inherits Strip's gutter
	     (--strip-pad = --space-8) so the row aligns with the page rhythm. */
	.similar {
		margin-block-start: var(--space-8);
	}

	/* — Skeletons. */
	.hero-skeleton-img {
		inline-size: 100%;
		block-size: 100%;
		background: var(--ink-100);
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	.poster-skeleton {
		aspect-ratio: var(--poster-aspect);
		background: var(--ink-100);
		border-radius: var(--radius-card);
		transform: translateY(calc(-1 * var(--space-6)));
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	.line {
		block-size: 0.75rem;
		background: var(--ink-100);
		border-radius: 2px;
		margin-block-start: var(--space-3);
		animation: pulse 1.6s var(--ease-in-out) infinite;
	}
	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.55;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.hero-skeleton-img,
		.poster-skeleton,
		.line {
			animation: none;
		}
	}

	/* — States. */
	.state {
		margin: var(--space-7) var(--space-6) 0;
		padding: var(--space-6);
		border-inline-start: 2px solid var(--ink-300);
	}
	.state-error {
		border-inline-start-color: var(--accent-oxblood);
	}
	.state-headline {
		margin: 0 0 var(--space-2);
		font-family: var(--font-body);
		font-size: var(--type-display-m);
		font-weight: 600;
		color: var(--bone-100);
		letter-spacing: -0.01em;
	}
	.state-detail {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-body);
		color: var(--bone-300);
		max-inline-size: 60ch;
	}
</style>
