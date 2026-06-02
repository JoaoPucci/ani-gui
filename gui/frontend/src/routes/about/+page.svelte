<!--
  About — editorial register, sectioned like /settings. Pulls
  dependency + asset metadata from $lib/about/credits (hand-curated;
  bump whenever a runtime dep lands or its version moves). The
  donate block uses a pure ETH-address guard + MetaMask URL builder
  in $lib/about/eth so the constants stay testable.

  Reached via the version chip in the left rail's footer
  (see +layout.svelte's `.rail-foot` link), not a top-level rail
  entry — keeps the rail uncluttered while pairing version + about
  naturally.
-->
<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import {
		ASSETS,
		BUNDLED_TOOLS,
		DONATION_ETH_ADDRESS,
		type AssetNoteId,
		type BundledToolNoteId
	} from '$lib/about/credits';
	import { toastStore } from '$lib/toasts/store.svelte';
	import { APP_VERSION as appVersion } from '$lib/version';

	// Static switches so paraglide compiles each m.* into an actual
	// callable rather than getting reached by a dynamic indexer (the
	// build pipeline doesn't allow the latter). Adding an entry to
	// BUNDLED_TOOLS / ASSETS in credits.ts requires extending the
	// matching switch — the BundledToolNoteId / AssetNoteId union
	// keeps that obligation typechecked.
	function bundledToolNote(id: BundledToolNoteId): string {
		switch (id) {
			case 'ani_cli':
				return m.about_bundled_tool_note_ani_cli();
			case 'fzf':
				return m.about_bundled_tool_note_fzf();
			case 'aria2':
				return m.about_bundled_tool_note_aria2();
			case 'ffmpeg':
				return m.about_bundled_tool_note_ffmpeg();
		}
	}

	function assetNote(id: AssetNoteId): string {
		switch (id) {
			case 'lottie_loading':
				return m.about_asset_note_lottie_loading();
		}
	}

	async function copyAddress() {
		try {
			// `navigator.clipboard` requires a secure context — Electron's
			// `app://localhost` origin satisfies that, and the dev server
			// at http://localhost too. Older fallbacks (document.execCommand)
			// aren't worth carrying; modern Chromium has had this for years.
			await navigator.clipboard.writeText(DONATION_ETH_ADDRESS);
			toastStore.push({ kind: 'success', message: m.about_donate_copied_toast() });
		} catch {
			toastStore.push({ kind: 'error', message: m.about_donate_copy_failed_toast() });
		}
	}
</script>

<svelte:head>
	<title>{m.app_page_title_about()}</title>
</svelte:head>

<main class="page">
	<header class="page-head">
		<p class="eyebrow">
			<span class="eyebrow-key">{m.about_eyebrow_key()}</span>
		</p>
		<div class="brand-row">
			<svg
				class="brand-mark"
				viewBox="0 0 32 32"
				width="56"
				height="56"
				fill="none"
				aria-hidden="true"
			>
				<rect x="2" y="2" width="28" height="28" rx="6" fill="var(--brand)" />
				<rect x="6" y="6" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="6" y="11" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="6" y="16" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="6" y="21" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="23.5" y="6" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="23.5" y="11" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="23.5" y="16" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
				<rect x="23.5" y="21" width="2.5" height="2.5" rx="0.5" fill="var(--brand-ink)" />
			</svg>
			<div class="brand-text">
				<h1 class="page-title">{m.about_title()}</h1>
				<p class="tagline">{m.about_tagline()}</p>
				<p class="version">
					<span class="version-key">{m.about_version_prefix()}</span>
					<span class="version-val">{appVersion}</span>
				</p>
			</div>
		</div>
		<p class="intro">{m.about_intro()}</p>
	</header>

	<!-- DONATE -->
	<section class="section">
		<h2 class="section-eyebrow">
			<span>{m.about_section_donate_title()}</span>
			<span class="section-eyebrow-faint">{m.about_section_donate_hint()}</span>
		</h2>
		<p class="section-intro">{m.about_donate_intro()}</p>
		<div class="donate-row">
			<button
				type="button"
				class="donate-address"
				aria-label={m.about_donate_copy_button_aria()}
				title={m.about_donate_copy_label()}
				onclick={copyAddress}
			>
				<span class="donate-address-key">
					<!-- Ethereum diamond mark. Two-shape silhouette using
					     currentColor so the icon tracks the surrounding
					     brand-coloured "ETH" text. viewBox is tightened
					     to the actual glyph bounds (x≈4.58→19.42, y=0→24)
					     so the icon has zero internal padding — otherwise
					     the visual left edge of the button drifts ~3px
					     inward relative to the symmetric box padding. -->
					<svg
						class="donate-address-icon"
						viewBox="4.58 0 14.84 24"
						width="10"
						height="16"
						fill="currentColor"
						aria-hidden="true"
					>
						<path
							d="M11.944 17.97L4.58 13.62 11.944 24l7.37-10.38-7.376 4.35h.006zM12.056 0L4.69 12.223l7.365 4.354 7.365-4.354L12.056 0z"
						/>
					</svg>
					ETH
				</span>
				<span class="donate-address-val">{DONATION_ETH_ADDRESS}</span>
				<span class="donate-address-action" aria-hidden="true">{m.about_donate_copy_label()}</span>
			</button>
		</div>
	</section>

	<!-- ASSETS -->
	<section class="section">
		<h2 class="section-eyebrow">
			<span>{m.about_section_assets_title()}</span>
			<span class="section-eyebrow-faint">{m.about_section_assets_hint()}</span>
		</h2>
		<ul class="credit-list">
			{#each ASSETS as asset (asset.url)}
				<li class="credit-item">
					<div class="credit-head">
						<!-- eslint-disable svelte/no-navigation-without-resolve -->
						<a class="credit-name" href={asset.url} target="_blank" rel="noopener noreferrer"
							>{asset.name}</a
						>
						<!-- eslint-enable svelte/no-navigation-without-resolve -->
						<span class="credit-license">{asset.license}</span>
					</div>
					<p class="credit-note">
						{assetNote(asset.noteId)}
						{#if asset.authorUrl}
							<!-- eslint-disable svelte/no-navigation-without-resolve -->
							<a
								class="credit-author credit-author-link"
								href={asset.authorUrl}
								target="_blank"
								rel="noopener noreferrer">— {asset.author}</a
							>
							<!-- eslint-enable svelte/no-navigation-without-resolve -->
						{:else}
							<span class="credit-author">— {asset.author}</span>
						{/if}
					</p>
				</li>
			{/each}
		</ul>
	</section>

	<!-- BUNDLED TOOLS -->
	<section class="section">
		<h2 class="section-eyebrow">
			<span>{m.about_section_bundled_tools_title()}</span>
			<span class="section-eyebrow-faint">{m.about_section_bundled_tools_hint()}</span>
		</h2>
		<ul class="dep-list" aria-label={m.about_section_bundled_tools_title()}>
			{#each BUNDLED_TOOLS as dep (dep.name)}
				<li class="dep-item">
					<!-- eslint-disable svelte/no-navigation-without-resolve -->
					<a class="dep-name" href={dep.url} target="_blank" rel="noopener noreferrer">{dep.name}</a
					>
					<!-- eslint-enable svelte/no-navigation-without-resolve -->
					<span class="dep-version">{dep.version ?? m.about_dep_version_unversioned()}</span>
					<span class="dep-license">{dep.license}</span>
					<span class="dep-note">{bundledToolNote(dep.noteId)}</span>
				</li>
			{/each}
		</ul>
	</section>

	<!-- LICENSE -->
	<section class="section">
		<h2 class="section-eyebrow">
			<span>{m.about_section_license_title()}</span>
			<span class="section-eyebrow-faint">{m.about_section_license_hint()}</span>
		</h2>
		<p class="section-intro">{m.about_license_body()}</p>
		<!-- eslint-disable svelte/no-navigation-without-resolve -->
		<a
			class="license-link"
			href={m.about_license_link_url()}
			target="_blank"
			rel="noopener noreferrer"
		>
			<!-- GitHub Octocat mark. Inlined as a single SVG path (per
			     GitHub's logo guidelines: their mark linking to their
			     site is permitted). Using currentColor so the icon
			     tracks the label's bone-100 / brand colour. -->
			<svg
				class="license-link-icon"
				viewBox="0 0 16 16"
				width="16"
				height="16"
				fill="currentColor"
				aria-hidden="true"
			>
				<path
					d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82a7.59 7.59 0 0 1 2-.27c.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"
				/>
			</svg>
			<span>{m.about_license_link_label()}</span>
		</a>
		<!-- eslint-enable svelte/no-navigation-without-resolve -->
	</section>
</main>

<style>
	.page {
		max-inline-size: 60rem;
		margin-inline: auto;
		padding-block: var(--space-7);
		padding-inline: var(--space-7);
		display: flex;
		flex-direction: column;
		gap: var(--space-7);
	}

	.page-head {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	.eyebrow {
		margin: 0;
		display: flex;
		align-items: center;
		gap: var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.eyebrow-key {
		color: var(--brand);
	}

	.brand-row {
		display: flex;
		align-items: center;
		gap: var(--space-5);
	}
	.brand-mark {
		flex: 0 0 56px;
	}
	.brand-text {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	.page-title {
		margin: 0;
		font-family: var(--font-display);
		font-size: var(--type-display-s);
		font-weight: 700;
		letter-spacing: -0.01em;
		color: var(--bone-100);
	}
	.tagline {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-body-l);
		color: var(--bone-200);
	}
	.version {
		margin: 0;
		display: inline-flex;
		gap: var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-300);
	}
	.version-key {
		text-transform: uppercase;
		letter-spacing: var(--tracking-micro);
	}
	.version-val {
		color: var(--bone-100);
	}
	.intro {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-body);
		line-height: 1.55;
		color: var(--bone-200);
		max-inline-size: 56ch;
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.section-eyebrow {
		margin: 0;
		display: flex;
		align-items: baseline;
		gap: var(--space-3);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-100);
		padding-block-end: var(--space-3);
		border-block-end: 1px solid var(--bone-500);
	}
	.section-eyebrow-faint {
		color: var(--bone-400);
		font-weight: 500;
	}
	.section-intro {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-body);
		line-height: 1.5;
		color: var(--bone-200);
		max-inline-size: 56ch;
	}

	/* Donate */
	.donate-row {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3);
		align-items: stretch;
	}
	.donate-address {
		flex: 1 1 24rem;
		display: grid;
		grid-template-columns: auto 1fr auto;
		gap: var(--space-3);
		align-items: center;
		padding: var(--space-3) var(--space-4);
		background: color-mix(in oklab, var(--brand) 6%, var(--ink-100));
		border: 1px solid color-mix(in oklab, var(--brand) 25%, var(--bone-500));
		border-radius: var(--radius-card, 8px);
		color: var(--bone-100);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		cursor: pointer;
		text-align: start;
		transition:
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.donate-address:hover {
		background: color-mix(in oklab, var(--brand) 12%, var(--ink-100));
		border-color: color-mix(in oklab, var(--brand) 45%, var(--bone-500));
	}
	.donate-address:focus-visible {
		outline: none;
		box-shadow:
			0 0 0 2px var(--ink-000),
			0 0 0 4px var(--brand);
	}
	.donate-address-key {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: var(--tracking-micro);
		color: var(--brand);
	}
	.donate-address-icon {
		flex: 0 0 auto;
	}
	.donate-address-val {
		font-feature-settings:
			'tnum' 1,
			'ss01' 1;
		font-size: var(--type-body);
		word-break: break-all;
	}
	.donate-address-action {
		font-size: var(--type-micro);
		text-transform: uppercase;
		letter-spacing: var(--tracking-micro);
		color: var(--bone-300);
		font-weight: 600;
	}
	/* Assets credit list */
	.credit-list {
		margin: 0;
		padding: 0;
		list-style: none;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.credit-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		padding-block: var(--space-3);
		border-block-end: 1px solid var(--bone-600);
	}
	.credit-item:last-child {
		border-block-end: none;
	}
	.credit-head {
		display: flex;
		align-items: baseline;
		gap: var(--space-3);
		flex-wrap: wrap;
	}
	.credit-name {
		font-family: var(--font-body);
		font-size: var(--type-body);
		font-weight: 600;
		color: var(--bone-100);
		text-decoration: none;
		border-block-end: 1px solid transparent;
		transition: border-color var(--dur-fast) var(--ease-out-soft);
	}
	.credit-name:hover {
		border-block-end-color: var(--brand);
	}
	.credit-license {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		text-transform: uppercase;
		letter-spacing: var(--tracking-micro);
		color: var(--bone-300);
	}
	.credit-note {
		margin: 0;
		font-family: var(--font-body);
		font-size: var(--type-meta);
		color: var(--bone-300);
		line-height: 1.5;
	}
	.credit-author {
		color: var(--bone-400);
	}
	.credit-author-link {
		text-decoration: none;
		border-block-end: 1px solid transparent;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.credit-author-link:hover {
		color: var(--bone-200);
		border-block-end-color: var(--brand);
	}

	/* Dependency list — table-style grid for scannability. Three
	   layout breakpoints: stacked on narrow viewports, 3-col on
	   medium (name / version / license + note below), 4-col on wide. */
	.dep-list {
		margin: 0;
		padding: 0;
		list-style: none;
		display: flex;
		flex-direction: column;
	}
	.dep-item {
		display: grid;
		grid-template-columns:
			minmax(8rem, max-content) minmax(4rem, max-content) minmax(5rem, max-content)
			1fr;
		column-gap: var(--space-4);
		row-gap: var(--space-1);
		padding-block: var(--space-3);
		border-block-end: 1px solid var(--bone-600);
		align-items: baseline;
	}
	.dep-item:last-child {
		border-block-end: none;
	}
	.dep-name {
		font-family: var(--font-mono);
		font-size: var(--type-body);
		font-weight: 600;
		color: var(--bone-100);
		text-decoration: none;
		border-block-end: 1px solid transparent;
		transition: border-color var(--dur-fast) var(--ease-out-soft);
	}
	.dep-name:hover {
		border-block-end-color: var(--brand);
	}
	.dep-version {
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-200);
		font-feature-settings: 'tnum' 1;
	}
	.dep-license {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		text-transform: uppercase;
		letter-spacing: var(--tracking-micro);
		color: var(--bone-300);
	}
	.dep-note {
		font-family: var(--font-body);
		font-size: var(--type-meta);
		color: var(--bone-300);
		line-height: 1.5;
	}
	@media (max-inline-size: 720px) {
		.dep-item {
			grid-template-columns: 1fr auto;
			grid-template-areas:
				'name version'
				'license license'
				'note note';
		}
		.dep-name {
			grid-area: name;
		}
		.dep-version {
			grid-area: version;
		}
		.dep-license {
			grid-area: license;
		}
		.dep-note {
			grid-area: note;
		}
	}

	/* License link — pill with the donate-address treatment scaled
	   down (subtle brand-tinted fill + visible brand-tinted border at
	   rest). The original transparent-bg version read as misaligned
	   text; an inline underline-only version read as a label. This
	   sits between: clearly a clickable surface at rest, brand-
	   coloured on hover. */
	.license-link {
		align-self: flex-start;
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-4);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		font-weight: 600;
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		text-decoration: none;
		color: var(--bone-100);
		background: color-mix(in oklab, var(--brand) 6%, var(--ink-100));
		border: 1px solid color-mix(in oklab, var(--brand) 25%, var(--bone-500));
		border-radius: var(--radius-pill, 999px);
		transition:
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.license-link:hover {
		background: color-mix(in oklab, var(--brand) 14%, var(--ink-100));
		border-color: color-mix(in oklab, var(--brand) 50%, var(--bone-500));
	}
	.license-link:focus-visible {
		outline: none;
		box-shadow:
			0 0 0 2px var(--ink-000),
			0 0 0 4px var(--brand);
	}
</style>
