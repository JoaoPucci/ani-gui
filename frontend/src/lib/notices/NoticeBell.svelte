<!--
  The topbar bell and the panel it opens.

  The panel is a dialog, not a menu: a row holds up to three controls
  (its lead, its action, its dismiss), which ARIA does not allow inside
  a menu item. Tab walks every control; ↑/↓ move between rows and Home
  and End jump, landing on each row's lead; Escape closes and returns
  focus to the bell. It is non-modal: a press outside closes it.

  Opening marks every row read, as the download dock does; the rows
  that were new when it opened keep their marker while it stays open,
  so what was new is still visible on the look that clears it.

  The bell's count shows every unread row but the update while its own
  badge shows it. When the count goes from nothing to something the
  bell swings sideways once and the count pops onto it ($lib/notices/
  bell); under reduced motion it stays still, and the stylesheet rule
  in tokens.css stops the animation too. The count is oxblood while a
  failure or a live problem is unread, neutral otherwise.
-->
<script lang="ts">
	import { tick, untrack } from 'svelte';
	import { cubicOut } from 'svelte/easing';
	import { m } from '$lib/paraglide/messages';
	import Icon from '$lib/components/Icon.svelte';
	import { fly } from '$lib/motion/transitions';
	import { reducedMotion } from '$lib/motion/reduced-motion.svelte';
	import { updateStore } from '$lib/update/store.svelte';
	import NoticeRow from './NoticeRow.svelte';
	import { noticeCenter } from './instance';
	import { countLabel, shouldRing } from './bell';
	import { copyOf } from './copy';
	import { ordered, type Notice } from './model';
	import { act, stepRow } from './panel';

	let {
		onPlayer
	}: {
		/** On the player, rows open in place instead of navigating. */
		onPlayer: boolean;
	} = $props();

	let bell = $state<HTMLButtonElement | null>(null);
	let panel = $state<HTMLElement | null>(null);
	let snapshot = $state<Set<string>>(new Set());
	let ringing = $state(false);
	let lastCount = 0;
	let wasOpen = false;

	const updateOnBadge = $derived(updateStore.hasUpdate);
	const count = $derived(noticeCenter.count(updateOnBadge));
	const tone = $derived(noticeCenter.tone(updateOnBadge));
	const rows = $derived(ordered(noticeCenter.rows));
	const open = $derived(noticeCenter.open);

	$effect(() => {
		const now = count;
		untrack(() => {
			if (shouldRing(lastCount, now, reducedMotion.current)) ringing = true;
			lastCount = now;
		});
	});

	$effect(() => {
		const isOpen = noticeCenter.open;
		untrack(() => {
			if (isOpen && !wasOpen) {
				snapshot = new Set(noticeCenter.rows.filter((n) => !n.read).map((n) => n.key));
				noticeCenter.markAllRead();
				void focusIn();
			} else if (!isOpen && wasOpen) {
				snapshot = new Set();
				noticeCenter.expanded = null;
			}
			wasOpen = isOpen;
		});
	});

	async function focusIn() {
		await tick();
		if (!panel) return;
		const want = noticeCenter.expanded
			? panel.querySelector<HTMLElement>('[aria-expanded="true"]')
			: null;
		const first = panel.querySelector<HTMLElement>('[data-row] .nr-lead');
		(want ?? first ?? panel).focus();
	}

	function close(returnFocus: boolean) {
		noticeCenter.open = false;
		if (returnFocus) bell?.focus();
	}

	$effect(() => {
		if (!open) return;
		const onDown = (e: PointerEvent) => {
			const t = e.target as Node | null;
			if (!t || bell?.contains(t) || panel?.contains(t)) return;
			close(false);
		};
		const onKey = (e: KeyboardEvent) => {
			if (e.key !== 'Escape' || e.defaultPrevented) return;
			const inside = panel?.contains(document.activeElement) || bell === document.activeElement;
			e.preventDefault();
			close(Boolean(inside));
		};
		document.addEventListener('pointerdown', onDown);
		document.addEventListener('keydown', onKey);
		return () => {
			document.removeEventListener('pointerdown', onDown);
			document.removeEventListener('keydown', onKey);
		};
	});

	// Escape is the panel's everywhere: it closes the panel and goes no
	// further. On the player every key pressed in the panel is the
	// panel's, so none also seeks, changes the volume or toggles
	// fullscreen through the shortcuts listening on the document and the
	// window. Elsewhere other keys go on to the app's own shortcuts, and
	// the search ones (`/`, Ctrl/Cmd+K) close the panel as the search
	// takes the focus.
	function onPanelKey(e: KeyboardEvent) {
		// Escape is the panel's alone, so no other popover closes with it.
		if (onPlayer || e.key === 'Escape') e.stopPropagation();
		if (e.key === 'Escape') {
			e.preventDefault();
			close(true);
			return;
		}
		// The search shortcut takes over from the panel.
		const searchKey = e.key === '/' || ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k');
		if (!onPlayer && searchKey) {
			close(false);
			return;
		}
		if (!panel) return;
		const leads = Array.from(panel.querySelectorAll<HTMLElement>('[data-row] .nr-lead'));
		const at = leads.findIndex((el) => el.closest('[data-row]')?.contains(document.activeElement));
		const to = stepRow(e.key, at, leads.length);
		if (to === null) return;
		e.preventDefault();
		leads[to].focus();
	}

	function actOn(n: Notice) {
		const action = copyOf(n).action;
		if (!action) return;
		// The panel steps aside for whatever the action opens.
		close(false);
		act(action.kind, n, {
			openUpdate: () => updateStore.openDialog(),
			reveal: (dir) => void window.aniGui?.revealInFolder?.(dir)
		});
	}

	function markAll() {
		noticeCenter.markAllRead();
		snapshot = new Set();
		panel?.focus();
	}
</script>

<div class="nc-wrap">
	<button
		bind:this={bell}
		type="button"
		class="nc-bell tone-{tone}"
		class:open
		class:ringing
		aria-haspopup="dialog"
		aria-expanded={open}
		aria-controls={open ? 'nc-panel' : undefined}
		aria-label={count > 0 ? m.notices_bell_label_unread({ count }) : m.notices_bell_label()}
		title={m.notices_bell_label()}
		onclick={() => (noticeCenter.open = !noticeCenter.open)}
		onanimationend={(e) => {
			if (e.animationName.endsWith('nc-swing')) ringing = false;
		}}
	>
		<span class="nc-glyph"><Icon name="bell" size={22} /></span>
		{#if count > 0}
			<span class="nc-badge" aria-hidden="true">{countLabel(count)}</span>
		{/if}
	</button>
	{#if open}
		<div
			bind:this={panel}
			id="nc-panel"
			class="nc-pop"
			role="dialog"
			aria-labelledby="nc-title"
			tabindex="-1"
			onkeydown={onPanelKey}
			transition:fly={{ y: -6, duration: 180, easing: cubicOut }}
		>
			<header class="nc-head">
				<h2 id="nc-title" class="nc-title">{m.notices_panel_title()}</h2>
				{#if snapshot.size > 0}
					<button type="button" class="nc-textbtn" onclick={markAll}
						>{m.notices_mark_all_read()}</button
					>
				{/if}
			</header>
			{#if rows.length === 0}
				<div class="nc-empty">
					<p class="nc-empty-title">{m.notices_empty_title()}</p>
					<p class="nc-empty-body">{m.notices_empty_body()}</p>
				</div>
			{:else}
				<ul class="nc-list" aria-labelledby="nc-title">
					{#each rows as n (n.key)}
						<NoticeRow
							notice={n}
							unread={snapshot.has(n.key) || !n.read}
							inPlace={onPlayer}
							expanded={noticeCenter.expanded === n.key}
							onToggle={() =>
								(noticeCenter.expanded = noticeCenter.expanded === n.key ? null : n.key)}
							onFollow={() => close(false)}
							onAct={() => actOn(n)}
							onDismiss={() => noticeCenter.dismiss(n.key)}
						/>
					{/each}
				</ul>
			{/if}
		</div>
	{/if}
</div>

<style>
	.nc-wrap {
		position: relative;
	}
	/* The bell: the dock trigger's square, so the two read as one
	   family in the topbar. */
	.nc-bell {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		inline-size: 2.5rem;
		block-size: 2.5rem;
		padding: 0;
		border: 1px solid transparent;
		border-radius: var(--radius-card);
		background: transparent;
		color: var(--bone-300);
		cursor: pointer;
		transition:
			color var(--dur-fast) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
		-webkit-app-region: no-drag;
	}
	.nc-bell:hover {
		color: var(--bone-100);
		background: color-mix(in oklab, var(--bone-100) 6%, transparent);
	}
	.nc-bell:focus-visible {
		outline: 2px solid var(--brand);
		outline-offset: 2px;
	}
	.nc-bell.open {
		background: var(--ink-050);
		border-color: var(--ink-300);
		color: var(--bone-100);
	}
	.nc-glyph {
		display: inline-flex;
		transform-origin: 50% 12%;
	}
	/* The arrival: one sideways swing, hung from the crown, damped like
	   a real bell. Once per 0 → something; never a loop. */
	.ringing .nc-glyph {
		animation: nc-swing 760ms var(--ease-out-soft) 1;
	}
	@keyframes nc-swing {
		0% {
			transform: rotate(0deg);
		}
		15% {
			transform: rotate(16deg);
		}
		32% {
			transform: rotate(-13deg);
		}
		49% {
			transform: rotate(8deg);
		}
		66% {
			transform: rotate(-4deg);
		}
		83% {
			transform: rotate(1.5deg);
		}
		100% {
			transform: rotate(0deg);
		}
	}
	/* The count: mono numerals on a pill at the icon's corner. */
	.nc-badge {
		position: absolute;
		inset-block-start: 0.125rem;
		inset-inline-end: 0.0625rem;
		min-inline-size: 1.125rem;
		block-size: 1.125rem;
		padding-inline: 0.3125rem;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-pill);
		background: var(--ink-200);
		color: var(--bone-100);
		box-shadow:
			0 0 0 1px var(--ink-300),
			0 0 0 3px var(--ink-000);
		font-family: var(--font-mono);
		font-size: 0.6875rem;
		font-weight: 600;
		font-variant-numeric: tabular-nums;
		line-height: 1;
		pointer-events: none;
	}
	.ringing .nc-badge {
		animation: nc-pop 420ms var(--ease-out-soft) 90ms both;
	}
	@keyframes nc-pop {
		0% {
			transform: scale(0);
		}
		65% {
			transform: scale(1.22);
		}
		100% {
			transform: scale(1);
		}
	}
	/* Oxblood taken toward the ink so the bone numerals clear 4.5:1. */
	.tone-attention .nc-badge {
		background: color-mix(in srgb, var(--accent-oxblood) 75%, var(--ink-000));
		box-shadow: 0 0 0 2px var(--ink-000);
	}
	/* The panel: the dock's surface. It hangs from the bell's inline
	   end and is never wider than the window less its gutters, so it
	   neither clips nor overlaps the window controls at the smallest
	   window. The header holds still; only the list scrolls. */
	.nc-pop {
		position: absolute;
		inset-block-start: calc(100% + var(--space-2));
		inset-inline-end: 0;
		z-index: 50;
		display: flex;
		flex-direction: column;
		inline-size: min(25rem, calc(100vw - 2rem));
		max-block-size: min(34rem, calc(100vh - 6rem));
		background: var(--ink-050);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-card);
		box-shadow: 0 18px 36px -12px rgb(0 0 0 / 0.6);
		overflow: hidden;
		-webkit-app-region: no-drag;
	}
	.nc-pop:focus {
		outline: none;
	}
	.nc-head {
		display: flex;
		align-items: baseline;
		gap: var(--space-3);
		padding-block: var(--space-4) var(--space-3);
		padding-inline: var(--space-5) var(--space-4);
		border-block-end: 1px solid var(--ink-200);
	}
	.nc-title {
		margin: 0;
		margin-inline-end: auto;
		font-family: var(--font-display);
		font-size: var(--type-body-l);
		font-weight: 500;
		letter-spacing: var(--tracking-display);
		line-height: var(--leading-tight);
		color: var(--bone-100);
	}
	.nc-textbtn {
		padding: 0.125rem 0.25rem;
		border: 0;
		border-radius: 2px;
		background: none;
		font-size: var(--type-micro);
		font-weight: 500;
		color: var(--bone-300);
		text-align: end;
		cursor: pointer;
	}
	.nc-textbtn:hover {
		color: var(--bone-100);
		text-decoration: underline;
		text-underline-offset: 3px;
	}
	.nc-list {
		flex: 1 1 auto;
		min-block-size: 0;
		overflow-y: auto;
		overscroll-behavior: contain;
		list-style: none;
		margin: 0;
		padding: var(--space-2);
		display: flex;
		flex-direction: column;
		gap: 1px;
	}
	.nc-empty {
		padding: var(--space-6) var(--space-5);
		text-align: center;
	}
	.nc-empty-title {
		margin: 0 0 var(--space-2);
		font-family: var(--font-display);
		font-size: var(--type-body-l);
		color: var(--bone-200);
	}
	.nc-empty-body {
		margin: 0;
		font-size: var(--type-meta);
		line-height: 1.5;
		color: var(--bone-400);
	}
</style>
