/**
 * Hand-curated credits surfaced by the About page.
 *
 * Scope: the bits the About page is actually about — the upstream
 * binaries the packages ship so playback and downloads work without
 * the user installing anything, and editorial assets (Lottie
 * animation, eventually fonts / illustrations). Frontend + backend
 * dep lists are not credited here — they are dev-facing, and
 * surfacing them turned the page into a build dashboard rather than
 * an "about this app" surface.
 *
 * When a platform fetcher gains or loses an entry, or an asset is
 * swapped, update this list too. The obligation runs both ways: a
 * tool credited here that nothing spawns tells the reader the app
 * needs something it never invokes, which is how fzf and aria2c
 * outlived the script that used them.
 *
 * Display-only data (name / version / license / url) is hard-coded.
 * Visitor-facing description strings live in the i18n message
 * bundle and are looked up by the page via the `noteId`
 * discriminant. The data module never carries user-visible English
 * copy directly — that would defeat localization for the same
 * reason Paraglide exists.
 */

export type BundledToolNoteId = 'curl_impersonate' | 'yt_dlp' | 'ffmpeg';

export interface BundledTool {
	/** Display label — the upstream's own name. */
	name: string;
	/** Version string as it appears in the manifest. `null` when
	 *  the upstream doesn't version uniformly (ffmpeg is whatever
	 *  the distro ships). */
	version: string | null;
	/** SPDX license id (or a free-text combo for dual-licensed
	 *  upstreams). */
	license: string;
	/** Canonical upstream URL — repo or homepage. */
	url: string;
	/** Paraglide-key suffix the page maps to its localized
	 *  description. Adding an entry here means adding a matching
	 *  `about_bundled_tool_note_<noteId>` key in every locale's
	 *  about.json. */
	noteId: BundledToolNoteId;
}

/** Tools the packages bundle or recommend so playback and downloads
 *  work without the user installing anything by hand. Versions track
 *  what `fetch-linux-deps.mjs` / `fetch-windows-deps.mjs` pin — the
 *  two stage the same set, so one version here covers both. */
export const BUNDLED_TOOLS: BundledTool[] = [
	{
		// The transport the native resolver spawns. The provider
		// fingerprints TLS and answers a plain curl with an
		// interstitial, so without this nothing resolves at all.
		name: 'curl-impersonate',
		version: '2.0.0',
		license: 'MIT',
		url: 'https://github.com/lexiforest/curl-impersonate',
		noteId: 'curl_impersonate'
	},
	{
		// The downloader. Preferred over ffmpeg when both are present:
		// it retries fragments indefinitely and pulls sixteen at once,
		// where ffmpeg does one stream copy with no retries.
		name: 'yt-dlp',
		version: '2025.09.26',
		license: 'Unlicense',
		url: 'https://github.com/yt-dlp/yt-dlp',
		noteId: 'yt_dlp'
	},
	{
		name: 'ffmpeg',
		version: null,
		license: 'LGPL-2.1+ / GPL',
		url: 'https://ffmpeg.org/',
		noteId: 'ffmpeg'
	}
];

/** Assets the page credits separately from upstream binaries —
 *  illustrations, animations, fonts that aren't shipped as separate
 *  packages but are baked into the bundle. */
export type AssetNoteId = 'lottie_loading';

export interface AssetCredit {
	/** Display label — what the asset is, not where it lives. */
	name: string;
	/** Author or studio name. */
	author: string;
	/** Optional URL pointing at the author's profile / homepage. When
	 *  present, the page renders the author name as a link — gives
	 *  the creator a proper backlink in addition to the asset URL. */
	authorUrl?: string;
	/** SPDX license id where applicable; free-text where the source
	 *  uses a custom license. */
	license: string;
	/** Canonical source URL (the asset itself). */
	url: string;
	/** Paraglide-key suffix the page maps to its localized
	 *  description. */
	noteId: AssetNoteId;
}

export const ASSETS: AssetCredit[] = [
	{
		// LottieFiles' canonical title for this animation isn't exposed
		// outside their UI (the URL slug is just "loading"). Using a
		// descriptive label here rather than inventing a name; the URL
		// is the link of record.
		name: 'Loading animation (LottieFiles)',
		author: 'Pickyourtrail',
		authorUrl: 'https://lottiefiles.com/pickyourtrail',
		license: 'Lottie Simple License',
		url: 'https://lottiefiles.com/free-animation/loading-OkRMnK50fl',
		noteId: 'lottie_loading'
	}
];

/** Donation address — single source of truth for the donate block
 *  and the eth.test fixture. EIP-55 mixed-case for display. */
export const DONATION_ETH_ADDRESS = '0x097cD53Dc5Dda28c4f6A4431EA014916891beC02';
