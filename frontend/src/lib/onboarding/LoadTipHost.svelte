<!--
  The player's load tip, placed by the play page inside the player
  frame after its controls (so Tab reaches it there). It shows the tip
  the player has ($./player-tip) on a start that lasts a second
  ($./load-tip-session), never takes focus, is announced once in a
  polite live region, closes on Got it or Escape (unless a menu, a
  popover or a dialog takes Escape first), and when it leaves with
  focus in it hands focus to the player.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { isModalOpen, watchModals } from '$lib/components/modal-layer';
	import { m } from '$lib/paraglide/messages';
	import LoadTip from './LoadTip.svelte';
	import { LoadTipSession } from './load-tip-session.svelte';
	import { escapeIsTips, focusPlayer, playerTip } from './player-tip';
	import { onboarding } from './tips.svelte';
	import { stepWords } from './words';

	let {
		veil,
		blocked,
		playerMenuOpen,
		accent
	}: {
		/** The player's start veil. */
		veil: 'off' | 'start' | 'stall';
		/** Fullscreen, or the error panel, is up. */
		blocked: boolean;
		/** One of the player's own menus is open. */
		playerMenuOpen: boolean;
		accent: string;
	} = $props();

	let root = $state<HTMLDivElement | undefined>();
	let announcement = $state('');
	let modalOpen = $state(false);
	// The tip this visit shows, fixed once it is chosen so a close does
	// not swap in another.
	let shown = $state<ReturnType<typeof playerTip>>(null);
	const words = $derived(shown ? stepWords(shown.tip.id, 1) : null);

	const session = new LoadTipSession({
		now: () => performance.now(),
		setTimer: (fn, ms) => setTimeout(fn, ms),
		clearTimer: (t) => clearTimeout(t),
		onSeen: () => {
			if (shown && !shown.asked) onboarding.markSeen(shown.tip.id);
		},
		onGone: () => {
			announcement = '';
			const frame = root?.closest<HTMLElement>('.player-frame');
			if (frame && root?.contains(document.activeElement)) focusPlayer(frame);
		}
	});

	// Choose the tip before a start begins; once chosen it stays.
	$effect(() => {
		if (shown || session.state.phase !== 'idle') return;
		const next = playerTip(onboarding.tips, onboarding.armed);
		if (next && stepWords(next.tip.id, 1)) shown = next;
	});

	$effect(() => {
		if (!shown) return;
		session.feed({ veil, blocked: blocked || modalOpen });
	});

	// Announced once, as it appears; an armed preview is used up then.
	let said = false;
	$effect(() => {
		if (!session.up || said || !shown || !words) return;
		said = true;
		if (shown.asked) onboarding.disarm();
		announcement = m.onboarding_announce({ version: shown.tip.since, title: words.title() });
	});

	onMount(() => {
		void onboarding.load();
		const onKey = (e: KeyboardEvent) => {
			if (!session.up || !escapeIsTips(e, playerMenuOpen, document)) return;
			e.preventDefault();
			session.close();
		};
		document.addEventListener('keydown', onKey);
		return () => {
			document.removeEventListener('keydown', onKey);
			session.stop();
		};
	});

	onMount(() => {
		modalOpen = isModalOpen();
		return watchModals(() => (modalOpen = isModalOpen()));
	});
</script>

<div bind:this={root} class="load-tip-host" data-onboard="player-frame">
	<p class="sr-only" role="status">{announcement}</p>
	{#if shown && words && session.up}
		<LoadTip
			title={words.title()}
			body={words.body()}
			version={shown.tip.since}
			newLabel={m.onboarding_new()}
			{accent}
			phase={session.state.phase === 'playing' ? 'playing' : 'loading'}
			gotItLabel={m.onboarding_got_it()}
			closeAria={m.onboarding_close_tip()}
			onDismiss={() => session.close()}
		/>
	{/if}
</div>

<style>
	.load-tip-host {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}
	.sr-only {
		position: absolute;
		inline-size: 1px;
		block-size: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
</style>
