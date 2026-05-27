<!--
  UpdateBadge — small topbar button that appears (with brand-orange
  glow) when a newer release is available on GitHub. Sits to the
  left of the DownloadDock icon. Click → opens UpdateDialog and
  retires the glow for the seen tag.
-->
<script lang="ts">
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import Icon from '$lib/components/Icon.svelte';
	import { m } from '$lib/paraglide/messages';
	import { updateStore } from '$lib/update/store.svelte';

	const visible = $derived(updateStore.hasUpdate);
	const tag = $derived(updateStore.available?.tag ?? '');
</script>

{#if visible}
	<button
		type="button"
		class="update-badge"
		title={m.update_badge_tooltip({ tag })}
		aria-label={m.update_badge_aria_label()}
		onclick={() => updateStore.openDialog()}
		transition:fly={{ y: 6, duration: 360, delay: 700, easing: cubicOut }}
	>
		<span class="update-badge-glyph"><Icon name="update" size={22} /></span>
		<span class="update-badge-led" aria-hidden="true"></span>
	</button>
{/if}

<style>
	.update-badge {
		/* Circular hit area sized to hug the icon. No background or
		   border by default — the icon stands on its own, no
		   button-frame look. Hover adds a subtle circular tint to
		   signal interactivity; focus-visible gives the keyboard
		   user a ring. */
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 2rem;
		block-size: 2rem;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: transparent;
		color: var(--bone-200);
		cursor: pointer;
		transition:
			background-color 160ms ease,
			color 160ms ease,
			transform 120ms ease;
	}
	/* One-shot entrance pop — fires the first time the badge renders.
	   The `fly` transition handles the slide-in; this scale-bounce
	   drags the eye after the slide settles. Delay matches the
	   transition's delay so the bounce starts the moment the badge
	   has finished sliding into place. */
	.update-badge {
		animation: update-badge-pop 520ms var(--ease-out-soft, cubic-bezier(0.2, 0.8, 0.2, 1)) 1000ms
			both;
	}
	@keyframes update-badge-pop {
		0% {
			transform: scale(0.7);
		}
		60% {
			transform: scale(1.12);
		}
		100% {
			transform: scale(1);
		}
	}
	.update-badge:hover {
		background: color-mix(in oklab, var(--bone-100) 8%, transparent);
		color: var(--bone-100);
	}
	.update-badge:active {
		transform: scale(0.94);
	}
	.update-badge:focus-visible {
		outline: 2px solid var(--brand);
		outline-offset: 2px;
	}

	/* Neon-style glow. Applied as filter: drop-shadow on the icon
	   wrapper rather than box-shadow on the button, so the halo
	   follows the icon's circular silhouette instead of the button's
	   bounding rect — no visible square frame. */
	.update-badge-glyph {
		display: inline-flex;
		transition: filter 240ms ease;
	}
	/* LED notification dot at the top-right corner of the icon.
	   Brand orange, persistently pulsing so the user keeps noticing
	   there's something to act on. Glows via box-shadow to read as
	   a luminous indicator rather than a flat sticker. */
	.update-badge-led {
		position: absolute;
		inset-block-start: 0.25rem;
		inset-inline-end: 0.25rem;
		inline-size: 0.55rem;
		block-size: 0.55rem;
		border-radius: 50%;
		background: var(--brand);
		box-shadow:
			0 0 0 2px var(--ink-100, var(--ink-050)),
			0 0 6px var(--brand),
			0 0 12px color-mix(in oklab, var(--brand) 60%, transparent);
		animation: update-badge-led-pulse 1.6s ease-in-out infinite;
		pointer-events: none;
	}
	@keyframes update-badge-led-pulse {
		0%,
		100% {
			opacity: 0.85;
			transform: scale(0.9);
			box-shadow:
				0 0 0 2px var(--ink-100, var(--ink-050)),
				0 0 4px var(--brand),
				0 0 8px color-mix(in oklab, var(--brand) 50%, transparent);
		}
		50% {
			opacity: 1;
			transform: scale(1.15);
			box-shadow:
				0 0 0 2px var(--ink-100, var(--ink-050)),
				0 0 8px var(--brand),
				0 0 16px color-mix(in oklab, var(--brand) 75%, transparent);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.update-badge {
			animation: none;
		}
		.update-badge-led {
			animation: none;
		}
	}
</style>
