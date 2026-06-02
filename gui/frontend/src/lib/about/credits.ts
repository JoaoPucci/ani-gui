/**
 * Hand-curated credits list for the About page.
 *
 * Lives here (not in a build-time-generated artifact) because the
 * runtime dependency tree is small enough to maintain by hand and
 * the page wants the editorialised version: one line per dep, with
 * a stable display label, license, and upstream URL.
 *
 * When you add a runtime dep to `gui/frontend/package.json` or to
 * the `[dependencies]` block of `gui/backend/Cargo.toml`, add it
 * here too. Dev/test-only deps (vitest, eslint, prettier, etc.) and
 * transitive-only crates are out of scope — the list is what
 * ani-gui ships, not its toolchain.
 *
 * Donation address is also exported from here so the page and any
 * test fixture import from a single source of truth.
 */

export interface CreditEntry {
	/** Display label — usually the package name. Keep stable; this
	 *  is what users will quote when they ask about a dep. */
	name: string;
	/** Version string as it appears in the manifest. `null` when the
	 *  upstream doesn't version (e.g., the bundled ani-cli script
	 *  carries its own tag separately). */
	version: string | null;
	/** SPDX license id (e.g. "MIT", "Apache-2.0", "GPL-3.0"). */
	license: string;
	/** Canonical upstream URL — repo or homepage. */
	url: string;
	/** One-line note on what the dep does or why it's bundled. */
	note: string;
}

/** Frontend runtime deps from `gui/frontend/package.json`'s
 *  `dependencies` block plus SvelteKit/Svelte themselves (which sit
 *  under devDependencies for the bundler but ship inside the
 *  compiled output). i18n + types-only packages are omitted. */
export const FRONTEND_DEPS: CreditEntry[] = [
	{
		name: 'Svelte',
		version: '5.55.5',
		license: 'MIT',
		url: 'https://svelte.dev/',
		note: 'Reactive UI runtime — the renderer is a Svelte 5 app.'
	},
	{
		name: 'SvelteKit',
		version: '2.59.1',
		license: 'MIT',
		url: 'https://svelte.dev/docs/kit/',
		note: 'App router and build pipeline.'
	},
	{
		name: 'Vite',
		version: '8.0.10',
		license: 'MIT',
		url: 'https://vite.dev/',
		note: 'Dev server and bundler.'
	},
	{
		name: 'hls.js',
		version: '1.6.16',
		license: 'Apache-2.0',
		url: 'https://github.com/video-dev/hls.js',
		note: 'HLS playlist + segment playback for the embedded player.'
	},
	{
		name: 'lottie-web',
		version: '5.13.0',
		license: 'MIT',
		url: 'https://github.com/airbnb/lottie-web',
		note: 'Renders the loading animation.'
	},
	{
		name: 'Paraglide JS',
		version: '2.18.0',
		license: 'Apache-2.0',
		url: 'https://inlang.com/m/gerre34r/library-inlang-paraglideJs',
		note: 'Compile-time i18n message bundles.'
	}
];

/** Backend runtime deps from `gui/backend/Cargo.toml`'s
 *  `[dependencies]` block. Dev-dependencies (proptest, wiremock,
 *  tokio-test) are omitted. Major crates only — small utility
 *  crates (futures-util, bytes, etc.) are bundled into the binary
 *  but don't carry attribution weight on their own. */
export const BACKEND_DEPS: CreditEntry[] = [
	{
		name: 'tokio',
		version: '1.41',
		license: 'MIT',
		url: 'https://github.com/tokio-rs/tokio',
		note: 'Async runtime.'
	},
	{
		name: 'axum',
		version: '0.7',
		license: 'MIT',
		url: 'https://github.com/tokio-rs/axum',
		note: 'HTTP server for the streaming proxy and IPC routes.'
	},
	{
		name: 'tower-http',
		version: '0.6',
		license: 'MIT',
		url: 'https://github.com/tower-rs/tower-http',
		note: 'CORS + tracing middleware for the axum router.'
	},
	{
		name: 'reqwest',
		version: '0.12',
		license: 'MIT OR Apache-2.0',
		url: 'https://github.com/seanmonstar/reqwest',
		note: 'HTTP client (rustls, no openssl).'
	},
	{
		name: 'rusqlite',
		version: '0.32',
		license: 'MIT',
		url: 'https://github.com/rusqlite/rusqlite',
		note: 'SQLite bindings for the metadata cache.'
	},
	{
		name: 'serde',
		version: '1.0',
		license: 'MIT OR Apache-2.0',
		url: 'https://serde.rs/',
		note: 'Serialisation backbone for IPC + config.'
	},
	{
		name: 'tracing',
		version: '0.1',
		license: 'MIT',
		url: 'https://github.com/tokio-rs/tracing',
		note: 'Structured logging with daily rolling file output.'
	},
	{
		name: 'm3u8-rs',
		version: '6.0',
		license: 'MIT',
		url: 'https://github.com/rutgersc/m3u8-rs',
		note: 'HLS playlist parser/rewriter.'
	}
];

/** Tools the .deb / AppImage bundles or recommends so ani-cli's
 *  `dep_ch` finds them without the user having to install anything
 *  by hand. The lifetime of these versions matches what
 *  `fetch-linux-deps.mjs` pins. */
export const BUNDLED_TOOLS: CreditEntry[] = [
	{
		name: 'ani-cli',
		version: null,
		license: 'GPL-3.0',
		url: 'https://github.com/pystardust/ani-cli',
		note: 'Source-resolution and download pipeline. ani-gui is a desktop shell over the bundled script.'
	},
	{
		name: 'fzf',
		version: '0.62.0',
		license: 'MIT',
		url: 'https://github.com/junegunn/fzf',
		note: 'Bundled in .deb + AppImage so ani-cli always has a selector available.'
	},
	{
		name: 'aria2',
		version: '1.37.0',
		license: 'GPL-2.0',
		url: 'https://aria2.github.io/',
		note: 'Bundled in .deb + AppImage; ani-cli uses it for parallel downloads.'
	},
	{
		name: 'ffmpeg',
		version: null,
		license: 'LGPL-2.1+ / GPL',
		url: 'https://ffmpeg.org/',
		note: '.deb declares it as Recommends; AppImage relies on the system install. Used to mux downloaded segments into MP4.'
	}
];

/** Assets the page credits separately from the dep tree —
 *  illustrations, animations, fonts that aren't shipped as npm
 *  packages but are baked into the bundle. */
export interface AssetCredit {
	/** Display label — what the asset is, not where it lives. */
	name: string;
	/** Author or studio name. */
	author: string;
	/** SPDX license id where applicable. Free-text where the source
	 *  uses a custom license. */
	license: string;
	/** Canonical source URL. */
	url: string;
	/** One-line note explaining where this asset appears in the app. */
	note: string;
}

export const ASSETS: AssetCredit[] = [
	{
		name: 'Liquid Loader (Lottie)',
		author: 'LottieFiles community',
		license: 'LottieFiles Free License',
		url: 'https://lottiefiles.com/free-animation/loading-OkRMnK50fl',
		note: 'Drives the LoadingOverlay used during play resolution and similar waits.'
	}
];

/** Donation address — single source of truth for the donate block
 *  and the eth.test fixture. EIP-55 mixed-case for display. */
export const DONATION_ETH_ADDRESS = '0x097cD53Dc5Dda28c4f6A4431EA014916891beC02';
