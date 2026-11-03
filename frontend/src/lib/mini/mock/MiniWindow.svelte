<!-- The mini window's own surface, drawn over the video the host
     holds: the drag strip, the controls in either layout, the captions
     menu, the on-top hint, the handover still, and the states a
     stream can reach while it is mini. The decisions live in this
     folder's .ts modules; this is their adapter. -->
<script module lang="ts">
	export type MiniSim = 'none' | 'stall' | 'error' | 'end';
	export interface MiniSteps {
		prev(): void;
		next(): void;
		hasPrev(): boolean;
		hasNext(): boolean;
		/** Puts the play page's player into its error state. */
		fail(): void;
	}
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import { m } from '$lib/paraglide/messages';
	import { formatTime } from '$lib/play/format';
	import { reducedMotion } from '$lib/motion/reduced-motion.svelte';
	import {
		MINI_IDLE_HIDE_MS,
		miniChromeShown,
		miniFadeMs,
		miniGripShown,
		miniStripShown
	} from './chrome';
	import { decideMiniKeyAction } from './keys';
	import { cueLifts, cueSizePx, miniControlSet } from './fit';
	import { PICTURE_CLICK_DELAY_MS, decidePictureClick, seekFraction } from './picture';
	import { onTopCopy, onTopView } from './on-top';
	import type { MiniMockPrefs } from './prefs';
	import type { MiniSession } from './session';

	let {
		session,
		prefs,
		still,
		steps,
		width,
		height,
		sim,
		skip,
		onReturn,
		onClose,
		onDragStart,
		onResizeStart,
		onClearSim,
		onSkipDone,
		onHintSeen
	}: {
		session: MiniSession;
		prefs: MiniMockPrefs;
		still: string | null;
		steps: MiniSteps | null;
		width: number;
		height: number;
		sim: MiniSim;
		skip: boolean;
		onReturn: () => void;
		onClose: () => void;
		onDragStart: (e: PointerEvent) => void;
		onResizeStart: (e: PointerEvent) => void;
		onClearSim: () => void;
		onSkipDone: () => void;
		onHintSeen: () => void;
	} = $props();

	const video = $derived(session.video);
	let root = $state<HTMLDivElement | undefined>();

	// The element's state, mirrored.
	let paused = $state(true);
	let currentTime = $state(0);
	let duration = $state(0);
	let bufferedEnd = $state(0);
	let muted = $state(false);
	let volume = $state(1);
	interface TrackRow {
		track: TextTrack;
		label: string;
	}
	let tracks = $state.raw<TrackRow[]>([]);
	let showing = $state<TextTrack | null>(null);
	let cueRises = $state(true);

	$effect(() => {
		const v = video;
		const sync = () => {
			paused = v.paused;
			currentTime = v.currentTime;
			duration = Number.isFinite(v.duration) ? v.duration : 0;
			bufferedEnd = v.buffered.length ? v.buffered.end(v.buffered.length - 1) : 0;
			muted = v.muted;
			volume = v.volume;
		};
		const readTracks = () => {
			const rows: TrackRow[] = [];
			let on: TextTrack | null = null;
			for (const t of Array.from(v.textTracks)) {
				if (t.kind !== 'subtitles' && t.kind !== 'captions') continue;
				rows.push({ track: t, label: t.label || t.language });
				if (t.mode === 'showing') on = t;
			}
			tracks = rows;
			showing = on;
		};
		sync();
		readTracks();
		const events = ['play', 'pause', 'timeupdate', 'durationchange', 'progress', 'volumechange'];
		for (const e of events) v.addEventListener(e, sync);
		v.textTracks.addEventListener('change', readTracks);
		v.textTracks.addEventListener('addtrack', readTracks);
		v.textTracks.addEventListener('removetrack', readTracks);
		return () => {
			for (const e of events) v.removeEventListener(e, sync);
			v.textTracks.removeEventListener('change', readTracks);
			v.textTracks.removeEventListener('addtrack', readTracks);
			v.textTracks.removeEventListener('removetrack', readTracks);
		};
	});
	// Whether the cue on screen rises with the chrome, read as cues
	// change on the showing track.
	$effect(() => {
		const t = showing;
		if (!t) return;
		const readCue = () => {
			const cue = t.activeCues?.[0] as VTTCue | undefined;
			cueRises = cue ? cueLifts(cue.line === 'auto' ? 'auto' : cue.line, cue.snapToLines) : true;
		};
		readCue();
		t.addEventListener('cuechange', readCue);
		return () => t.removeEventListener('cuechange', readCue);
	});

	// The handover gap: the main window's last frame, under the
	// loading treatment, until the stream would have landed.
	let loading = $state(false);
	onMount(() => {
		root?.focus({ preventScroll: true });
		const gap = Number(prefs.gap);
		if (gap <= 0) return;
		const wasPlaying = !video.paused;
		video.pause();
		loading = true;
		const t = setTimeout(() => {
			loading = false;
			if (wasPlaying) void video.play().catch(() => {});
		}, gap);
		return () => clearTimeout(t);
	});

	// Chrome: shown by pointer movement or focus, gone after the idle
	// time, or 600 ms after the pointer leaves the window.
	let pointerActive = $state(true);
	let focusWithin = $state(false);
	let timelineHover = $state(false);
	let ccOpen = $state(false);
	let idle: ReturnType<typeof setTimeout> | null = null;
	function wake(ms = MINI_IDLE_HIDE_MS) {
		pointerActive = true;
		if (idle) clearTimeout(idle);
		idle = setTimeout(() => (pointerActive = false), ms);
	}
	onMount(() => {
		wake();
		return () => {
			if (idle) clearTimeout(idle);
		};
	});

	const set = $derived(miniControlSet(width, prefs.layout));
	const view = $derived(onTopView(prefs.platform, prefs.onTop, prefs.hintSeen));
	// Which explainer: the GNOME menu's steps only once appendix step 4
	// confirms them; the switch shows each text for review.
	const copyKind = $derived(
		onTopCopy(prefs.copy === 'kde' ? 'KDE' : 'GNOME', prefs.copy === 'gnome')
	);
	const copy = $derived(
		copyKind === 'gnome'
			? m.mini_ontop_gnome()
			: copyKind === 'kde'
				? m.mini_ontop_kde()
				: m.mini_ontop_other()
	);
	// The pin: on Wayland it explains, and is dropped at the compact
	// size; elsewhere it toggles keep-on-top, which starts on.
	const pinShown = $derived(view.pin !== null && !(view.pin === 'explain' && set.compact));
	let pinPressed = $state(true);
	let popoverOpen = $state(false);
	// Opened by itself on the first open, the explainer is soft: it
	// leaves with the chrome and counts as seen. Opened from the pin,
	// it stays until dismissed.
	let popoverSoft = $state(false);
	const cardShown = $derived(view.card && prefs.onTop === 'card');
	onMount(() => {
		if (view.card && prefs.onTop === 'both' && !set.compact) {
			popoverOpen = true;
			popoverSoft = true;
		}
	});
	function dismissHint() {
		popoverOpen = false;
		popoverSoft = false;
		if (view.card) onHintSeen();
	}
	function closeMenus() {
		ccOpen = false;
		if (popoverOpen) dismissHint();
	}

	const overlay = $derived(sim === 'error' || sim === 'end' || cardShown);
	const chromeOn = $derived(
		miniChromeShown({
			pointerActive,
			focusWithin,
			paused: paused && !loading,
			timelineHover,
			menuOpen: ccOpen || (popoverOpen && !popoverSoft),
			cardShown: overlay
		})
	);
	const stripOn = $derived(miniStripShown(chromeOn, sim === 'stall'));
	const gripOn = $derived(
		miniGripShown({ platform: prefs.platform, grip: prefs.grip, chromeShown: chromeOn })
	);
	$effect(() => {
		if (popoverSoft && !chromeOn) dismissHint();
	});
	const fadeMs = $derived(miniFadeMs(reducedMotion.current));
	const bottomOn = $derived(chromeOn && !loading && !overlay);
	const lifted = $derived(prefs.lift === 'lift' && bottomOn && cueRises);
	const cueSize = $derived(cueSizePx(height));
	// A control clicked with the mouse keeps focus; when the bottom
	// controls go inert it would drop to the page and the window's
	// keys with it. Focus returns to the window first.
	$effect(() => {
		if (bottomOn || !root) return;
		const active = document.activeElement;
		if (active && root.querySelector('.mw-bottom')?.contains(active)) {
			root.focus({ preventScroll: true });
		}
	});
	// The video sits in the host beside this root: it reads the cue
	// size, the lift and the fade from there.
	$effect(() => {
		const host = root?.parentElement;
		if (!host) return;
		host.style.setProperty('--mw-cue-size', `${cueSize}px`);
		host.style.setProperty('--mw-cue-lift', `${set.cueLift}px`);
		host.style.setProperty('--mw-fade', `${fadeMs}ms`);
	});

	// Click the picture: play or pause; double-click: back to the app.
	let clickTimer: ReturnType<typeof setTimeout> | null = null;
	let glyph = $state<'play' | 'pause' | null>(null);
	let glyphTimer: ReturnType<typeof setTimeout> | null = null;
	function togglePlay() {
		if (loading) return;
		if (video.paused) void video.play().catch(() => {});
		else video.pause();
		glyph = video.paused ? 'pause' : 'play';
		if (glyphTimer) clearTimeout(glyphTimer);
		glyphTimer = setTimeout(() => (glyph = null), 400);
	}
	function onPictureClick() {
		if (decidePictureClick(ccOpen || popoverOpen) === 'closeMenus') {
			closeMenus();
			return;
		}
		if (clickTimer) clearTimeout(clickTimer);
		clickTimer = setTimeout(togglePlay, PICTURE_CLICK_DELAY_MS);
	}
	function onPictureDouble() {
		if (clickTimer) clearTimeout(clickTimer);
		onReturn();
	}

	function seekBy(delta: number) {
		if (!duration) return;
		video.currentTime = Math.max(0, Math.min(duration, video.currentTime + delta));
	}
	function seekToClient(e: PointerEvent, el: HTMLElement) {
		const r = el.getBoundingClientRect();
		const rtl = getComputedStyle(el).direction === 'rtl';
		if (duration) video.currentTime = seekFraction(e.clientX, r.left, r.width, rtl) * duration;
	}
	let dragging = false;
	function onScrubDown(e: PointerEvent) {
		const el = e.currentTarget as HTMLElement;
		el.setPointerCapture(e.pointerId);
		dragging = true;
		seekToClient(e, el);
	}
	function onScrubMove(e: PointerEvent) {
		if (dragging) seekToClient(e, e.currentTarget as HTMLElement);
	}
	function onScrubUp() {
		dragging = false;
	}
	function onScrubKey(e: KeyboardEvent) {
		if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
			e.preventDefault();
			e.stopPropagation();
			seekBy(e.key === 'ArrowLeft' ? -5 : 5);
		}
	}

	function pickTrack(t: TextTrack | null) {
		for (const row of tracks) row.track.mode = row.track === t ? 'showing' : 'hidden';
		showing = t;
		ccOpen = false;
	}

	function onKey(e: KeyboardEvent) {
		const t = e.target as HTMLElement | null;
		const action = decideMiniKeyAction({
			key: e.key,
			inField: false,
			inButton: !!t && (t.tagName === 'BUTTON' || t.getAttribute('role') === 'button'),
			modifier: e.ctrlKey || e.metaKey || e.altKey,
			repeat: e.repeat,
			menuOpen: ccOpen || popoverOpen
		});
		if (!action) return;
		e.preventDefault();
		e.stopPropagation();
		switch (action.kind) {
			case 'togglePlay':
				togglePlay();
				break;
			case 'seek':
				seekBy(action.deltaSeconds);
				wake();
				break;
			case 'volume':
				video.volume = Math.max(0, Math.min(1, video.volume + action.delta));
				wake();
				break;
			case 'mute':
				video.muted = !video.muted;
				wake();
				break;
			case 'next':
				if (steps?.hasNext()) steps.next();
				break;
			case 'prev':
				if (steps?.hasPrev()) steps.prev();
				break;
			case 'return':
				onReturn();
				break;
			case 'closeMenu':
				closeMenus();
				break;
		}
	}

	function onSkip() {
		seekBy(85);
		onSkipDone();
	}

	const fraction = $derived(duration ? currentTime / duration : 0);
	const bufferedFraction = $derived(duration ? Math.min(1, bufferedEnd / duration) : 0);
	const episodeLabel = $derived(
		m.play_episode_nav_current_label({ episode: String(session.show.episode) })
	);
</script>

{#snippet playIcon(size: number)}
	<svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true">
		{#if paused}
			<path d="M8 5v14l11-7z" fill="currentColor" />
		{:else}
			<path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z" fill="currentColor" />
		{/if}
	</svg>
{/snippet}

{#snippet scrubber(edge: boolean)}
	<div
		class="mw-scrub"
		class:mw-scrub-edge={edge}
		role="slider"
		tabindex="0"
		aria-label={m.play_controls_scrubber_aria_label()}
		aria-valuemin="0"
		aria-valuemax={Math.max(1, Math.floor(duration))}
		aria-valuenow={Math.floor(currentTime)}
		aria-valuetext={m.mini_time_of({
			current: formatTime(currentTime),
			total: formatTime(duration)
		})}
		onpointerdown={onScrubDown}
		onpointermove={onScrubMove}
		onpointerup={onScrubUp}
		onpointerenter={() => (timelineHover = true)}
		onpointerleave={() => (timelineHover = false)}
		onkeydown={onScrubKey}
	>
		<div class="mw-track">
			<div class="mw-buffered" style:inline-size="{bufferedFraction * 100}%"></div>
			<div class="mw-fill" style:inline-size="{fraction * 100}%"></div>
			<div class="mw-thumb" style:inset-inline-start="{fraction * 100}%"></div>
		</div>
	</div>
{/snippet}

{#snippet ccButton()}
	<div class="mw-cc-wrap">
		<button
			type="button"
			class="mw-btn"
			class:mw-cc-on={showing !== null}
			aria-haspopup="menu"
			aria-expanded={ccOpen}
			aria-label={m.play_controls_cc_aria_label()}
			title={showing ? m.play_controls_cc_on_title() : m.play_controls_cc_off_title()}
			onclick={() => (ccOpen = !ccOpen)}
		>
			<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
				<path
					d="M19 4H5a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2zm-8 7H9.5v-.5h-2v3h2V13H11v1a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v1zm7 0h-1.5v-.5h-2v3h2V13H18v1a1 1 0 0 1-1 1h-3a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v1z"
					fill="currentColor"
				/>
			</svg>
			{#if showing}<span class="mw-cc-rule" aria-hidden="true"></span>{/if}
		</button>
	</div>
{/snippet}

{#snippet muteButton()}
	<button
		type="button"
		class="mw-btn"
		aria-label={muted ? m.play_controls_unmute_aria_label() : m.play_controls_mute_aria_label()}
		onclick={() => (video.muted = !video.muted)}
	>
		<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
			{#if muted || volume === 0}
				<path
					d="M16.5 12A4.5 4.5 0 0 0 14 7.97v2.21l2.45 2.45c.03-.2.05-.41.05-.63zm2.5 0c0 .94-.2 1.82-.54 2.64l1.51 1.51A8.8 8.8 0 0 0 21 12c0-4.28-2.99-7.86-7-8.77v2.06c2.89.86 5 3.54 5 6.71zM4.27 3 3 4.27 7.73 9H3v6h4l5 5v-6.73l4.25 4.25c-.67.52-1.42.93-2.25 1.18v2.06a8.99 8.99 0 0 0 3.69-1.81L19.73 21 21 19.73l-9-9L4.27 3zM12 4 9.91 6.09 12 8.18V4z"
					fill="currentColor"
				/>
			{:else}
				<path
					d="M3 9v6h4l5 5V4L7 9H3zm13.5 3A4.5 4.5 0 0 0 14 7.97v8.05c1.48-.73 2.5-2.25 2.5-4.02zM14 3.23v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z"
					fill="currentColor"
				/>
			{/if}
		</svg>
	</button>
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	bind:this={root}
	class="mini-window"
	class:mw-chrome-on={chromeOn}
	class:mw-compact={set.compact}
	class:mw-centre={prefs.layout === 'centre'}
	class:mw-cue-floor={prefs.cues === 'floor'}
	class:mw-lifted={lifted}
	class:mw-loading={loading || sim === 'stall'}
	class:mw-dim={overlay}
	class:mw-strip-on={stripOn}
	class:mw-has-grip={gripOn}
	style:--mw-fade="{fadeMs}ms"
	style:--mw-cue-lift="{set.cueLift}px"
	style:--mw-cue-size="{cueSize}px"
	style:--accent={session.show.accent}
	tabindex="-1"
	onpointermove={() => wake()}
	onpointerenter={() => wake()}
	onpointerleave={() => wake(600)}
	onfocusin={(e) => (focusWithin = (e.target as Element).matches(':focus-visible'))}
	onfocusout={(e) => {
		if (!root?.contains(e.relatedTarget as Node | null)) focusWithin = false;
	}}
	onkeydown={onKey}
>
	{#if loading && still}
		<img class="mw-still" src={still} alt="" />
	{/if}

	<!-- The picture: a click plays or pauses, a double-click returns. -->
	<div
		class="mw-picture"
		role="presentation"
		onclick={onPictureClick}
		ondblclick={onPictureDouble}
	></div>

	{#if loading || sim === 'stall'}
		<div class="mw-spinner" role="status">
			<span class="mw-sr">{m.play_now_playing_loading()}</span>
			<span class="mw-spinner-track"><span class="mw-spinner-sweep"></span></span>
		</div>
	{/if}

	{#if glyph || (paused && !loading && sim === 'none')}
		<div class="mw-glyph" class:mw-glyph-flash={glyph !== null} aria-hidden="true">
			{#if (glyph ?? (paused ? 'pause' : 'play')) === 'pause'}
				<svg viewBox="0 0 24 24" width="28" height="28"
					><path d="M8 5v14l11-7z" fill="currentColor" /></svg
				>
			{:else}
				<svg viewBox="0 0 24 24" width="28" height="28"
					><path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z" fill="currentColor" /></svg
				>
			{/if}
		</div>
	{/if}

	<div class="mw-scrim-top" aria-hidden="true"></div>
	<div class="mw-scrim-bottom" aria-hidden="true"></div>

	<!-- The drag strip: the window's only handle. -->
	<div class="mw-strip" onpointerdown={(e) => onDragStart(e)} role="presentation">
		{#if sim === 'stall'}
			<span class="mw-title mw-status" role="status">{m.mini_reconnecting()}</span>
		{:else}
			<span class="mw-title" title={session.show.title}>{session.show.title}</span>
		{/if}
		<span class="mw-ep">{episodeLabel}</span>
		<span class="mw-spacer"></span>
		{#if pinShown}
			<button
				type="button"
				class="mw-strip-btn"
				class:mw-pin-on={view.pin === 'toggle' && pinPressed}
				aria-pressed={view.pin === 'toggle' ? pinPressed : undefined}
				aria-expanded={view.pin === 'explain' ? popoverOpen : undefined}
				aria-label={view.pin === 'toggle' ? m.mini_pin_toggle() : m.mini_pin_explain()}
				title={view.pin === 'toggle' ? m.mini_pin_toggle() : m.mini_pin_explain()}
				onpointerdown={(e) => e.stopPropagation()}
				onclick={() => {
					if (view.pin === 'toggle') pinPressed = !pinPressed;
					else if (popoverOpen) dismissHint();
					else popoverOpen = true;
				}}
			>
				<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
					<path
						d="M16 9V4h1V2H7v2h1v5c0 1.66-1.34 3-3 3v2h5.97v7l1 1 1-1v-7H19v-2c-1.66 0-3-1.34-3-3z"
						fill="currentColor"
					/>
				</svg>
			</button>
		{/if}
		<button
			type="button"
			class="mw-strip-btn"
			aria-label={m.mini_back_title()}
			title={m.mini_back_title()}
			onpointerdown={(e) => e.stopPropagation()}
			onclick={onReturn}
		>
			<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
				<path
					d="M19 19H5V5h7V3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7h-2zM14 3v2h3.59l-9.83 9.83 1.41 1.41L19 6.41V10h2V3z"
					fill="currentColor"
				/>
			</svg>
		</button>
		<button
			type="button"
			class="mw-strip-btn mw-close"
			aria-label={m.mini_close()}
			title={m.mini_close()}
			onpointerdown={(e) => e.stopPropagation()}
			onclick={onClose}
		>
			<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
				<path
					d="M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
					fill="currentColor"
				/>
			</svg>
		</button>
	</div>

	{#if popoverOpen}
		<div class="mw-popover" role="dialog" aria-label={m.mini_ontop_heading()}>
			<p>{copy}</p>
			<button type="button" class="mw-text-btn" onclick={dismissHint}>{m.mini_got_it()}</button>
		</div>
	{/if}

	{#if cardShown}
		<div class="mw-panel" role="dialog" aria-label={m.mini_ontop_heading()}>
			<p class="mw-panel-heading">{m.mini_ontop_heading()}</p>
			<p class="mw-panel-body">{copy}</p>
			<div class="mw-panel-actions">
				<button type="button" class="mw-action mw-action-primary" onclick={dismissHint}
					>{m.mini_got_it()}</button
				>
			</div>
		</div>
	{:else if sim === 'error'}
		<div class="mw-panel" role="alert">
			<p class="mw-panel-heading">{m.play_error_stream_headline()}</p>
			<div class="mw-panel-actions">
				<button type="button" class="mw-action mw-action-primary" onclick={onClearSim}
					>{m.play_error_stream_reload()}</button
				>
				<button type="button" class="mw-action" onclick={onReturn}>{m.mini_open_in_app()}</button>
			</div>
		</div>
	{:else if sim === 'end'}
		<div class="mw-panel" role="status">
			<p class="mw-panel-heading">{m.mini_series_end()}</p>
			<div class="mw-panel-actions">
				<button type="button" class="mw-action" onclick={onReturn}>{m.mini_back()}</button>
			</div>
		</div>
	{/if}

	{#if skip && sim === 'none' && !loading}
		<button type="button" class="mw-skip" onclick={onSkip}>
			<span aria-hidden="true">⏭</span>
			<span class="mw-skip-label">{m.play_skip_op()}</span>
		</button>
	{/if}

	<!-- Bottom controls. -->
	<div class="mw-bottom" inert={!bottomOn}>
		{#if prefs.layout === 'bar'}
			{#if set.compact}
				<div class="mw-row">
					<button
						type="button"
						class="mw-btn"
						aria-label={paused
							? m.play_controls_play_aria_label()
							: m.play_controls_pause_aria_label()}
						onclick={togglePlay}>{@render playIcon(20)}</button
					>
					{@render scrubber(false)}
					<span class="mw-time">{formatTime(currentTime)}</span>
					{@render ccButton()}
				</div>
			{:else}
				<div class="mw-timeline">
					{@render scrubber(false)}
					<span class="mw-time"
						>{formatTime(currentTime)}<span class="mw-time-sep">
							/ {formatTime(duration)}</span
						></span
					>
				</div>
				<div class="mw-row">
					<button
						type="button"
						class="mw-btn"
						aria-label={m.play_episode_nav_previous_aria_label()}
						disabled={!steps?.hasPrev()}
						onclick={() => steps?.prev()}
					>
						<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
							<path d="M6 6h2v12H6zm3.5 6 8.5 6V6z" fill="currentColor" />
						</svg>
					</button>
					<button
						type="button"
						class="mw-btn"
						aria-label={paused
							? m.play_controls_play_aria_label()
							: m.play_controls_pause_aria_label()}
						onclick={togglePlay}>{@render playIcon(22)}</button
					>
					<button
						type="button"
						class="mw-btn"
						aria-label={m.play_episode_nav_next_aria_label()}
						disabled={!steps?.hasNext()}
						onclick={() => steps?.next()}
					>
						<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
							<path d="M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z" fill="currentColor" />
						</svg>
					</button>
					<span class="mw-spacer"></span>
					{#if set.mute}{@render muteButton()}{/if}
					{@render ccButton()}
				</div>
			{/if}
		{:else}
			<div class="mw-corner">
				<span class="mw-time">{formatTime(currentTime)}</span>
				<span class="mw-spacer"></span>
				{#if set.time === 'both'}
					<span class="mw-time mw-time-sep">−{formatTime(Math.max(0, duration - currentTime))}</span
					>
				{/if}
				{#if set.mute}{@render muteButton()}{/if}
				{@render ccButton()}
			</div>
			{@render scrubber(true)}
		{/if}
	</div>

	{#if prefs.layout === 'centre'}
		<div class="mw-cluster" inert={!bottomOn}>
			<button
				type="button"
				class="mw-round mw-round-sm"
				aria-label={m.mini_back_10()}
				onclick={() => seekBy(-10)}
			>
				<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
					<path
						d="M12 5V1L7 6l5 5V7a6 6 0 1 1-6 6H4a8 8 0 1 0 8-8z M10.9 16h-.85v-3.26l-1 .31v-.69l1.76-.62h.09V16z m4.28-1.76c0 .32-.03.6-.1.82s-.17.42-.29.57-.28.26-.45.33-.37.1-.59.1-.41-.03-.59-.1-.33-.18-.46-.33-.23-.34-.3-.57-.11-.5-.11-.82v-.74c0-.32.03-.6.1-.82s.17-.42.29-.57.28-.26.45-.33.37-.1.59-.1.41.03.59.1.33.18.46.33.23.34.3.57.11.5.11.82v.74z"
						fill="currentColor"
					/>
				</svg>
			</button>
			<button
				type="button"
				class="mw-round"
				aria-label={paused ? m.play_controls_play_aria_label() : m.play_controls_pause_aria_label()}
				onclick={togglePlay}>{@render playIcon(28)}</button
			>
			<button
				type="button"
				class="mw-round mw-round-sm"
				aria-label={m.mini_forward_10()}
				onclick={() => seekBy(10)}
			>
				<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
					<path
						d="M12 5V1l5 5-5 5V7a6 6 0 1 0 6 6h2a8 8 0 1 1-8-8z M10.9 16h-.85v-3.26l-1 .31v-.69l1.76-.62h.09V16z m4.28-1.76c0 .32-.03.6-.1.82s-.17.42-.29.57-.28.26-.45.33-.37.1-.59.1-.41-.03-.59-.1-.33-.18-.46-.33-.23-.34-.3-.57-.11-.5-.11-.82v-.74c0-.32.03-.6.1-.82s.17-.42.29-.57.28-.26.45-.33.37-.1.59-.1.41.03.59.1.33.18.46.33.23.34.3.57.11.5.11.82v.74z"
						fill="currentColor"
					/>
				</svg>
			</button>
		</div>
	{/if}

	{#if ccOpen}
		<div class="mw-cc-menu" role="menu" aria-label={m.play_controls_cc_menu_heading()}>
			{#each tracks as row (row.track)}
				<button
					type="button"
					class="mw-cc-item"
					role="menuitemradio"
					aria-checked={showing === row.track}
					title={row.label}
					onclick={() => pickTrack(row.track)}
				>
					<span class="mw-check" aria-hidden="true">{showing === row.track ? '✓' : ''}</span>
					<span class="mw-cc-name">{row.label}</span>
				</button>
			{/each}
			<div class="mw-cc-divider" role="separator"></div>
			<button
				type="button"
				class="mw-cc-item"
				role="menuitemradio"
				aria-checked={showing === null}
				onclick={() => pickTrack(null)}
			>
				<span class="mw-check" aria-hidden="true">{showing === null ? '✓' : ''}</span>
				<span class="mw-cc-name">{m.play_controls_cc_off()}</span>
			</button>
		</div>
	{/if}

	{#if gripOn}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="mw-grip" title={m.mini_resize()} onpointerdown={(e) => onResizeStart(e)}></div>
	{/if}
</div>

<style>
	.mini-window {
		position: absolute;
		inset: 0;
		overflow: hidden;
		color: #fff;
		font-family: var(--font-body);
		outline: none !important;
		box-shadow: none !important;
		--mw-strip-h: 32px;
		--mw-bottom-h: 64px;
	}
	.mini-window.mw-compact {
		--mw-strip-h: 28px;
		--mw-bottom-h: 36px;
	}
	.mini-window.mw-centre {
		--mw-bottom-h: 30px;
	}
	.mini-window:not(.mw-chrome-on) {
		cursor: none;
	}

	/* Subtitles: a size floor, and the lift while the chrome shows. */
	/* The video is the host's, beside this window's root; these rules
	   reach it through the host. */
	/* The floor goes on the cue container, not on ::cue: Chromium lays
	   cues out from the container's size, which it sets inline from the
	   video's height. A ::cue size alone overlaps two cues on screen at
	   once; the container's, made to win, keeps them apart. */
	:global(.mini-sim:has(> .mini-window.mw-cue-floor) video::-webkit-media-text-track-container) {
		font-size: var(--mw-cue-size) !important;
	}
	:global(.mini-sim video::-webkit-media-text-track-container) {
		transition: transform var(--mw-fade) var(--ease-out-soft);
	}
	:global(.mini-sim:has(> .mini-window.mw-lifted) video::-webkit-media-text-track-container) {
		transform: translateY(calc(-1 * var(--mw-cue-lift)));
	}

	.mw-still {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: contain;
		background: #000;
		opacity: 0.5;
	}
	:global(.mini-sim:has(> .mini-window.mw-loading) video) {
		opacity: 0.5;
	}
	:global(.mini-sim:has(> .mini-window.mw-dim) video) {
		opacity: 0.3;
	}
	/* No cues under the handover still or a card. */
	:global(
		.mini-sim:has(> .mini-window:is(.mw-loading, .mw-dim)) video::-webkit-media-text-track-container
	) {
		visibility: hidden;
	}
	.mw-picture {
		position: absolute;
		inset: 0;
	}

	.mw-spinner {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		pointer-events: none;
	}
	.mw-spinner-track {
		position: relative;
		display: block;
		inline-size: clamp(4rem, 30%, 8rem);
		block-size: 2px;
		border-radius: var(--radius-pill);
		background: color-mix(in oklab, var(--bone-100) 16%, transparent);
		box-shadow: 0 0 0 1px rgb(0 0 0 / 0.45);
		overflow: hidden;
	}
	.mw-spinner-sweep {
		position: absolute;
		inset-block: 0;
		inline-size: 40%;
		background: linear-gradient(
			to right,
			transparent,
			var(--accent) 35%,
			color-mix(in oklab, var(--accent), white 35%) 50%,
			var(--accent) 65%,
			transparent
		);
		animation: mw-sweep 1.4s var(--ease-in-out) infinite;
	}
	@keyframes mw-sweep {
		from {
			inset-inline-start: -40%;
		}
		to {
			inset-inline-start: 100%;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.mw-spinner-sweep {
			inset-inline-start: 0;
			inline-size: 100%;
			background: var(--accent);
			opacity: 0.6;
		}
	}
	.mw-sr {
		position: absolute;
		inline-size: 1px;
		block-size: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}

	.mw-glyph {
		position: absolute;
		inset-block-start: 50%;
		inset-inline-start: 50%;
		display: grid;
		place-items: center;
		inline-size: 56px;
		block-size: 56px;
		margin: -28px 0 0 -28px;
		border-radius: 50%;
		background: rgb(0 0 0 / 0.55);
		color: #fff;
		pointer-events: none;
	}
	.mw-compact .mw-glyph {
		inline-size: 44px;
		block-size: 44px;
		margin: -22px 0 0 -22px;
	}
	.mw-centre .mw-glyph {
		display: none;
	}
	.mw-glyph-flash {
		animation: mw-glyph var(--dur-fast) var(--ease-out-soft);
	}
	@keyframes mw-glyph {
		from {
			opacity: 0;
			scale: 0.9;
		}
	}

	/* Chrome layers: one fade for all of them. */
	.mw-scrim-top,
	.mw-scrim-bottom,
	.mw-strip,
	.mw-bottom,
	.mw-cluster,
	.mw-grip {
		opacity: 0;
		pointer-events: none;
		transition: opacity var(--mw-fade) var(--ease-out-soft);
	}
	.mw-strip-on .mw-scrim-top,
	.mw-strip-on .mw-strip,
	.mw-has-grip .mw-grip {
		opacity: 1;
		pointer-events: auto;
	}
	.mw-chrome-on .mw-scrim-bottom,
	.mw-chrome-on .mw-bottom:not([inert]),
	.mw-chrome-on .mw-cluster:not([inert]) {
		opacity: 1;
	}
	.mw-chrome-on .mw-bottom:not([inert]) {
		pointer-events: auto;
	}
	.mw-scrim-top {
		position: absolute;
		inset-inline: 0;
		inset-block-start: 0;
		block-size: 64px;
		background: linear-gradient(180deg, rgb(0 0 0 / 0.75) 0, transparent 100%);
		pointer-events: none !important;
	}
	.mw-scrim-bottom {
		position: absolute;
		inset-inline: 0;
		inset-block-end: 0;
		block-size: calc(var(--mw-bottom-h) + 8px);
		background: linear-gradient(
			180deg,
			transparent 0,
			rgb(0 0 0 / 0.6) 35%,
			rgb(0 0 0 / 0.85) 100%
		);
		pointer-events: none !important;
	}
	.mw-loading .mw-scrim-bottom,
	.mw-dim .mw-scrim-bottom {
		opacity: 0;
	}

	.mw-strip {
		position: absolute;
		inset-inline: 0;
		inset-block-start: 0;
		block-size: var(--mw-strip-h);
		padding: 4px 4px 4px var(--space-3);
		display: flex;
		align-items: center;
		gap: var(--space-2);
		cursor: grab;
		user-select: none;
	}
	.mw-title {
		flex: 0 1 auto;
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-weight: 600;
		font-size: 0.8125rem;
		line-height: 1.2;
		color: var(--bone-100);
		text-shadow: 0 1px 2px rgb(0 0 0 / 0.6);
	}
	.mw-status {
		color: var(--bone-200);
	}
	.mw-ep {
		flex: none;
		font-weight: 500;
		font-size: 0.75rem;
		font-variant-numeric: tabular-nums;
		color: var(--bone-200);
		text-shadow: 0 1px 2px rgb(0 0 0 / 0.6);
	}
	.mw-spacer {
		flex: 1 1 0;
	}
	.mw-strip-btn,
	.mw-btn {
		display: inline-grid;
		place-items: center;
		flex: none;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: transparent;
		color: #fff;
		cursor: pointer;
		transition: background var(--dur-fast) var(--ease-out-soft);
	}
	.mw-strip-btn {
		inline-size: 28px;
		block-size: 28px;
	}
	.mw-compact .mw-strip-btn {
		inline-size: 26px;
		block-size: 26px;
	}
	.mw-btn {
		position: relative;
		inline-size: 32px;
		block-size: 32px;
	}
	.mw-compact .mw-btn {
		inline-size: 28px;
		block-size: 28px;
	}
	.mw-strip-btn:hover,
	.mw-btn:hover:not(:disabled) {
		background: rgb(255 255 255 / 0.18);
	}
	.mw-btn:disabled {
		opacity: 0.4;
		cursor: default;
	}
	.mw-close:hover {
		background: color-mix(in oklab, var(--accent-oxblood) 80%, transparent);
	}
	.mw-strip-btn[aria-pressed='false'] {
		color: rgb(255 255 255 / 0.7);
	}
	.mw-pin-on {
		color: #fff;
		background: rgb(255 255 255 / 0.22);
	}

	.mw-bottom {
		position: absolute;
		inset-inline: 0;
		inset-block-end: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: var(--space-1) var(--space-2) 6px;
	}
	.mw-centre .mw-bottom {
		padding: 0;
		gap: 0;
	}
	/* The grip owns the bottom-end corner; the row stops short of it. */
	.mw-has-grip .mw-row,
	.mw-has-grip .mw-corner {
		padding-inline-end: 24px;
	}
	.mw-timeline,
	.mw-row,
	.mw-corner {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.mw-timeline {
		block-size: 20px;
		gap: var(--space-3);
		padding-inline: var(--space-1);
	}
	.mw-row {
		block-size: 32px;
		gap: 2px;
	}
	.mw-compact .mw-row {
		gap: var(--space-2);
	}
	.mw-corner {
		block-size: 26px;
		padding-inline: var(--space-2);
		gap: 2px;
	}
	.mw-time {
		flex: none;
		font-weight: 500;
		font-size: 0.75rem;
		line-height: 1;
		font-variant-numeric: tabular-nums;
		color: #fff;
		text-shadow: 0 1px 2px rgb(0 0 0 / 0.6);
	}
	.mw-time-sep {
		margin-inline-start: 0.3em;
		color: var(--bone-200);
	}

	.mw-scrub {
		flex: 1;
		min-inline-size: 0;
		display: flex;
		align-items: center;
		block-size: 20px;
		cursor: pointer;
	}
	.mw-scrub:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 3px;
		border-radius: 2px;
	}
	.mw-scrub-edge {
		flex: none;
		block-size: 12px;
		align-items: flex-end;
	}
	.mw-track {
		position: relative;
		inline-size: 100%;
		block-size: 3px;
		border-radius: 999px;
		background: color-mix(in oklab, var(--bone-100) 18%, transparent);
		transition: block-size var(--dur-fast) var(--ease-out-soft);
	}
	.mw-scrub-edge .mw-track {
		border-radius: 0;
	}
	.mw-scrub:hover .mw-track,
	.mw-scrub:focus-visible .mw-track {
		block-size: 5px;
	}
	.mw-scrub-edge:hover .mw-track {
		block-size: 6px;
	}
	.mw-buffered,
	.mw-fill {
		position: absolute;
		inset-block: 0;
		inset-inline-start: 0;
		border-radius: inherit;
	}
	.mw-buffered {
		background: rgb(255 255 255 / 0.35);
	}
	.mw-fill {
		background: var(--accent);
	}
	.mw-thumb {
		position: absolute;
		inset-block-start: 50%;
		inline-size: 10px;
		block-size: 10px;
		margin-inline-start: -5px;
		border-radius: 50%;
		background: var(--accent);
		transform: translateY(-50%) scale(0);
		transition: transform var(--dur-fast) var(--ease-out-soft);
	}
	.mw-scrub:hover .mw-thumb,
	.mw-scrub:focus-visible .mw-thumb {
		transform: translateY(-50%) scale(1);
	}

	.mw-cc-wrap {
		position: relative;
	}
	.mw-cc-rule {
		position: absolute;
		inset-inline: calc(50% - 0.5rem);
		inset-block-end: 4px;
		block-size: 2px;
		border-radius: 1px;
		background: var(--accent);
	}
	.mw-cc-menu {
		position: absolute;
		inset-inline-end: var(--space-2);
		inset-block-end: calc(var(--mw-bottom-h) + 4px);
		max-block-size: calc(100% - var(--mw-strip-h) - var(--mw-bottom-h) - 12px);
		min-inline-size: 10rem;
		max-inline-size: min(16rem, calc(100% - 16px));
		overflow-y: auto;
		overscroll-behavior: contain;
		scrollbar-width: thin;
		padding: 4px;
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-card);
		background: color-mix(in oklab, var(--ink-100) 97%, transparent);
		box-shadow: 0 8px 20px rgb(0 0 0 / 0.5);
		animation: mw-menu var(--mw-fade) var(--ease-out-soft);
	}
	@keyframes mw-menu {
		from {
			opacity: 0;
			translate: 0 4px;
		}
	}
	.mw-cc-item {
		display: flex;
		align-items: center;
		gap: 6px;
		inline-size: 100%;
		min-block-size: 28px;
		padding: 6px 10px;
		border: 0;
		border-radius: 4px;
		background: transparent;
		color: var(--bone-100);
		font: 500 0.8125rem/1.2 var(--font-body);
		text-align: start;
		cursor: pointer;
	}
	.mw-cc-item:hover {
		background: var(--ink-200);
	}
	.mw-check {
		flex: none;
		inline-size: 1rem;
		color: var(--accent);
	}
	.mw-cc-name {
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.mw-cc-divider {
		block-size: 1px;
		margin: 4px 0;
		background: var(--ink-200);
	}

	.mw-cluster {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-5);
		pointer-events: none;
	}
	.mw-chrome-on .mw-cluster {
		pointer-events: none;
	}
	.mw-compact .mw-cluster {
		gap: var(--space-4);
	}
	.mw-round {
		display: grid;
		place-items: center;
		inline-size: 56px;
		block-size: 56px;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: rgb(0 0 0 / 0.45);
		color: #fff;
		cursor: pointer;
		pointer-events: auto;
	}
	.mw-round-sm {
		inline-size: 40px;
		block-size: 40px;
	}
	.mw-compact .mw-round {
		inline-size: 44px;
		block-size: 44px;
	}
	.mw-compact .mw-round-sm {
		inline-size: 32px;
		block-size: 32px;
	}
	.mw-round:hover {
		background: rgb(0 0 0 / 0.65);
	}

	.mw-popover {
		position: absolute;
		inset-inline-end: var(--space-2);
		inset-block-start: calc(var(--mw-strip-h) + 4px);
		max-inline-size: min(17rem, calc(100% - 16px));
		max-block-size: calc(100% - var(--mw-strip-h) - 12px);
		overflow-y: auto;
		padding: var(--space-3);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-card);
		background: color-mix(in oklab, var(--ink-100) 97%, transparent);
		box-shadow: 0 8px 20px rgb(0 0 0 / 0.5);
		color: var(--bone-100);
		animation: mw-menu var(--mw-fade) var(--ease-out-soft);
	}
	.mw-popover p {
		margin: 0 0 var(--space-2);
		font-size: 0.8125rem;
		line-height: 1.45;
	}
	.mw-text-btn {
		display: block;
		margin-inline-start: auto;
		padding: 4px 8px;
		border: 0;
		border-radius: var(--radius-control);
		background: transparent;
		color: var(--bone-100);
		text-decoration: underline 1px;
		text-underline-offset: 3px;
		font: 600 0.8125rem/1 var(--font-body);
		cursor: pointer;
	}
	.mw-text-btn:hover {
		background: var(--ink-200);
	}

	.mw-panel {
		position: absolute;
		inset: 0;
		padding-block-start: var(--mw-strip-h);
		background: rgb(0 0 0 / 0.55);
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		padding-inline: var(--space-5);
		padding-block-end: var(--space-3);
		text-align: center;
	}
	.mw-panel-heading {
		margin: 0;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
		font-weight: 600;
		font-size: 0.9375rem;
		line-height: 1.3;
		color: var(--bone-100);
		text-wrap: balance;
	}
	.mw-panel-body {
		margin: 0;
		max-inline-size: 22rem;
		font-size: 0.8125rem;
		line-height: 1.45;
		color: var(--bone-200);
	}
	.mw-panel-actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: var(--space-2);
	}
	.mw-action {
		block-size: 32px;
		padding: 0 var(--space-3);
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-control);
		background: transparent;
		color: var(--bone-100);
		font: 600 0.8125rem/1 var(--font-body);
		cursor: pointer;
	}
	.mw-compact .mw-action {
		block-size: 28px;
	}
	.mw-action-primary {
		border-color: transparent;
		background: var(--bone-100);
		color: var(--ink-000);
	}

	.mw-skip {
		position: absolute;
		inset-inline-end: var(--space-3);
		inset-block-end: var(--space-3);
		display: inline-flex;
		align-items: center;
		gap: 6px;
		max-inline-size: 55%;
		block-size: 28px;
		padding: 0 var(--space-3);
		border: 1px solid color-mix(in oklab, var(--accent) 70%, transparent);
		border-radius: var(--radius-pill);
		background: rgba(15, 15, 17, 0.92);
		box-shadow: 0 4px 12px rgb(0 0 0 / 0.45);
		color: #fff;
		font: 600 0.75rem/1 var(--font-body);
		cursor: pointer;
		transition:
			translate var(--mw-fade) var(--ease-out-soft),
			background var(--dur-fast) var(--ease-out-soft);
	}
	.mw-chrome-on .mw-skip {
		translate: 0 calc(-1 * var(--mw-bottom-h));
	}
	.mw-skip:hover {
		background: color-mix(in oklab, var(--accent) 92%, #000);
	}
	.mw-skip-label {
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.mw-grip {
		position: absolute;
		inset-inline-end: 0;
		inset-block-end: 0;
		inline-size: 24px;
		block-size: 24px;
		cursor: nwse-resize;
		background: linear-gradient(
			135deg,
			transparent 45%,
			color-mix(in oklab, var(--bone-300) 80%, transparent) 45% 52%,
			transparent 52% 62%,
			color-mix(in oklab, var(--bone-300) 80%, transparent) 62% 69%,
			transparent 69% 79%,
			color-mix(in oklab, var(--bone-300) 80%, transparent) 79% 86%,
			transparent 86%
		);
	}
	.mw-grip:dir(rtl) {
		cursor: nesw-resize;
	}
	/* The app's two-tone ring: ink, then the show's accent. */
	.mini-window :global(button:focus-visible) {
		outline: none;
		box-shadow: var(--ring);
	}
</style>
