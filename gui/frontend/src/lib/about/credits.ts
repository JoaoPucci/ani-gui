/**
 * Hand-curated credits surfaced by the About page.
 *
 * Scope: the bits the About page is actually about — bundled
 * upstream tools (ani-cli + the binaries we ship alongside it for
 * `dep_ch` to find) and editorial assets (Lottie animation,
 * eventually fonts / illustrations). Frontend + backend dep lists
 * are not credited here — they are dev-facing and surfacing them
 * here turned the page into a build dashboard rather than an
 * "about this app" surface.
 *
 * When you change `fetch-linux-deps.mjs` or swap an asset, update
 * the entry here too — but keep the list focused on things a user
 * has a reason to know about.
 *
 * Display-only data (name / version / license / url) is hard-coded.
 * Visitor-facing description strings live in the i18n message
 * bundle and are looked up by the page via the `noteId`
 * discriminant. The data module never carries user-visible English
 * copy directly — that would defeat localization for the same
 * reason Paraglide exists.
 */

export type BundledToolNoteId = 'ani_cli' | 'fzf' | 'aria2' | 'ffmpeg';

export interface BundledTool {
	/** Display label — the upstream's own name. */
	name: string;
	/** Version string as it appears in the manifest. `null` when
	 *  the upstream doesn't version uniformly (the bundled ani-cli
	 *  script carries its own tag separately; ffmpeg is whatever
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

/** Tools the .deb / AppImage bundles or recommends so ani-cli's
 *  `dep_ch` finds them without the user having to install anything
 *  by hand. Versions track what `fetch-linux-deps.mjs` pins. */
export const BUNDLED_TOOLS: BundledTool[] = [
	{
		name: 'ani-cli',
		version: null,
		license: 'GPL-3.0',
		url: 'https://github.com/pystardust/ani-cli',
		noteId: 'ani_cli'
	},
	{
		name: 'fzf',
		version: '0.62.0',
		license: 'MIT',
		url: 'https://github.com/junegunn/fzf',
		noteId: 'fzf'
	},
	{
		name: 'aria2',
		version: '1.37.0',
		license: 'GPL-2.0',
		url: 'https://aria2.github.io/',
		noteId: 'aria2'
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
