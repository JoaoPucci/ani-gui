<!--
	Custom titlebar window controls for the frameless Electron shell.

	The window is frameless (gui/electron/main.js `frame: false`) because
	under native Wayland/Ozone GNOME draws no server-side decorations and
	Chromium's own client-side buttons ignored GNOME's button-layout,
	landing on the left (electron/electron#48422). We render minimize /
	maximize / close ourselves, on the OS-correct side.

	Renders nothing outside Electron (browser-only dev has no
	`window.aniGui.windowControls`), so the topbar is unchanged there.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { windowControlsSide } from '$lib/window/controls';
	import * as m from '$lib/paraglide/messages';

	const wc = typeof window !== 'undefined' ? window.aniGui?.windowControls : undefined;
	const side = windowControlsSide(
		typeof window !== 'undefined' ? window.aniGui?.platform : undefined
	);

	let maximized = $state(false);

	onMount(() => {
		if (!wc) return;
		maximized = wc.isMaximized();
		// The WM can maximize/restore without our buttons (super+↑, tiling,
		// double-click the drag strip), so stay subscribed to the truth.
		return wc.onMaximizeChange((v) => (maximized = v));
	});
</script>

{#if wc}
	<div
		class="winctl"
		class:winctl--left={side === 'left'}
		role="group"
		aria-label={m.window_controls_group()}
	>
		<button
			class="winctl-btn"
			type="button"
			aria-label={m.window_minimize()}
			title={m.window_minimize()}
			onclick={() => wc.minimize()}
		>
			<svg width="13" height="13" viewBox="0 0 11 11" aria-hidden="true">
				<rect x="1" y="5" width="9" height="1" fill="currentColor" />
			</svg>
		</button>
		<button
			class="winctl-btn"
			type="button"
			aria-label={maximized ? m.window_restore() : m.window_maximize()}
			title={maximized ? m.window_restore() : m.window_maximize()}
			onclick={() => wc.toggleMaximize()}
		>
			{#if maximized}
				<svg width="13" height="13" viewBox="0 0 11 11" aria-hidden="true">
					<rect x="1.5" y="3.5" width="6" height="6" fill="none" stroke="currentColor" />
					<path d="M3.5 3.5 V1.5 H9.5 V7.5 H7.5" fill="none" stroke="currentColor" />
				</svg>
			{:else}
				<svg width="13" height="13" viewBox="0 0 11 11" aria-hidden="true">
					<rect x="1.5" y="1.5" width="8" height="8" fill="none" stroke="currentColor" />
				</svg>
			{/if}
		</button>
		<button
			class="winctl-btn winctl-btn--close"
			type="button"
			aria-label={m.window_close()}
			title={m.window_close()}
			onclick={() => wc.close()}
		>
			<svg width="13" height="13" viewBox="0 0 11 11" aria-hidden="true">
				<path d="M1.5 1.5 L9.5 9.5 M9.5 1.5 L1.5 9.5" stroke="currentColor" stroke-width="1" />
			</svg>
		</button>
	</div>
{/if}

<style>
	/* The buttons must stay clickable inside the draggable topbar. */
	.winctl {
		display: flex;
		align-self: stretch;
		align-items: stretch;
		-webkit-app-region: no-drag;
		app-region: no-drag;
		margin-inline-start: var(--space-3, 0.75rem);
		/* Sit near the window's top-right corner like a native titlebar:
		   cancel the topbar's block padding (height) and most of its trailing
		   inline padding (--space-8 desktop), leaving a small --space-2 gap so
		   the buttons aren't jammed against the very edge. */
		margin-inline-end: calc(var(--space-2, 0.5rem) - var(--space-8, 4rem));
		margin-block: calc(-1 * var(--space-4, 1rem));
	}
	/* macOS keeps controls on the left; flexbox order floats the group
	   ahead of the topbar's other trailing items, near the left edge. */
	.winctl--left {
		order: -1;
		margin-inline: calc(var(--space-2, 0.5rem) - var(--space-8, 4rem)) var(--space-3, 0.75rem);
	}

	.winctl-btn {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 3rem;
		padding: 0;
		border: none;
		background: transparent;
		color: var(--text-2, currentColor);
		cursor: pointer;
	}
	/* The click target spans the full (tall) bar height so the corner stays
	   hittable and flush, but the hover/focus highlight is a centered SQUARE
	   chip — a full-height fill reads as an awkward vertical bar. */
	.winctl-btn::before {
		content: '';
		position: absolute;
		inset-block-start: 50%;
		inset-inline-start: 50%;
		translate: -50% -50%;
		inline-size: 2.25rem;
		block-size: 2.25rem;
		border-radius: var(--radius-sm, 6px);
		background: transparent;
		transition: background 0.12s ease;
	}
	.winctl-btn svg {
		position: relative;
	}
	.winctl-btn:hover {
		color: var(--text-1, currentColor);
	}
	.winctl-btn:hover::before {
		background: color-mix(in srgb, currentColor 16%, transparent);
	}
	.winctl-btn:focus-visible {
		outline: none;
	}
	.winctl-btn:focus-visible::before {
		outline: 2px solid var(--accent, currentColor);
		outline-offset: 0;
	}
	.winctl-btn--close:hover {
		color: #fff;
	}
	.winctl-btn--close:hover::before {
		background: #e81123;
	}

	/* The topbar's trailing padding shrinks to --space-4 below 720px; match
	   it so the controls stay flush at the narrow breakpoint too. */
	@media (max-inline-size: 720px) {
		.winctl {
			margin-inline-end: calc(var(--space-2, 0.5rem) - var(--space-4, 1rem));
		}
		.winctl--left {
			margin-inline: calc(var(--space-2, 0.5rem) - var(--space-4, 1rem)) var(--space-3, 0.75rem);
		}
	}
</style>
