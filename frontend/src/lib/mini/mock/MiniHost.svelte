<!-- The mini-player mockup's host, mounted once by the root layout.

     It draws the simulated mini window — a box floating over the main
     window at the size and place the real window would take — and
     holds the one session ($lib/mini/mock/session). The play page
     hands it its own video element, so the picture and the native
     subtitles inside are the real ones. Nothing here renders while
     the Debug switch is off. -->
<script module lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { browserStorage } from '$lib/hero/prefs';
	import { placePlayerVideo } from '$lib/play/player-video';
	import { loadMiniMockPrefs, saveMiniMockPrefs, type MiniMockPrefs } from './prefs';
	import { createMiniSessions, type MiniSession } from './session';
	import type { MiniSim, MiniSteps } from './MiniWindow.svelte';

	const sessions = createMiniSessions();
	let prefs = $state(loadMiniMockPrefs(browserStorage()));
	let session = $state.raw<MiniSession | null>(null);
	let still = $state.raw<string | null>(null);
	let steps = $state.raw<MiniSteps | null>(null);
	let sim = $state<MiniSim>('none');
	let skip = $state(false);
	// Wayland: a click in the main window raises it over the mini,
	// which the app cannot prevent (the mini-player proposal, PR #345,
	// section 4). The mockup
	// shows that by hiding the simulated window until it is raised.
	let covered = $state(false);
	let slot: HTMLElement | null = null;
	sessions.subscribe((s) => {
		session = s;
		covered = false;
		if (!s) {
			steps = null;
			sim = 'none';
			skip = false;
		}
	});

	function searchOf(href: string): string {
		return new URL(href, window.location.origin).search;
	}

	/** The current frame, as a JPEG data URL; null when the frame
	 *  cannot be read (nothing decoded yet, or a tainted canvas). */
	function captureStill(video: HTMLVideoElement): string | null {
		try {
			if (!video.videoWidth) return null;
			const canvas = document.createElement('canvas');
			canvas.width = video.videoWidth;
			canvas.height = video.videoHeight;
			canvas.getContext('2d')?.drawImage(video, 0, 0);
			return canvas.toDataURL('image/jpeg', 0.8);
		} catch {
			return null;
		}
	}

	export const miniMock = {
		get prefs(): MiniMockPrefs {
			return prefs;
		},
		get session(): MiniSession | null {
			return session;
		},
		get still(): string | null {
			return still;
		},
		get steps(): MiniSteps | null {
			return steps;
		},
		get sim(): MiniSim {
			return sim;
		},
		get skip(): boolean {
			return skip;
		},
		get covered(): boolean {
			return covered;
		},
		setPrefs(patch: Partial<MiniMockPrefs>) {
			prefs = { ...prefs, ...patch };
			saveMiniMockPrefs(browserStorage(), prefs);
			// Turning the mockups off ends the session with them.
			if (!prefs.enabled) sessions.close();
		},
		setSim(next: MiniSim) {
			sim = next;
		},
		setSkip(on: boolean) {
			skip = on;
		},
		/** Whether `video` is the one the mini window holds. */
		holds(video: HTMLVideoElement): boolean {
			return sessions.holds(video);
		},
		/** Moves `s.video` into the mini window and starts the session.
		 *  The video moves before the page drops its frame, so it never
		 *  leaves the document and playback carries on. */
		open(s: MiniSession, pageSteps: MiniSteps | null): boolean {
			if (!slot) return false;
			still = captureStill(s.video);
			placePlayerVideo(s.video, slot);
			sessions.open(s);
			steps = pageSteps;
			return true;
		},
		/** The play page is leaving; the session keeps playing. */
		pageGone(video: HTMLVideoElement) {
			if (!sessions.holds(video)) return;
			sessions.pageGone(video);
			steps = null;
		},
		returnToApp() {
			const show = session?.show;
			if (sessions.returnToApp() !== 'navigate' || !show) return;
			// The play page's own address, read from page.url when the
			// mini opened; the rule cannot follow resolve() into a module
			// script.
			// eslint-disable-next-line svelte/no-navigation-without-resolve
			void goto(resolve('/play/[id]', { id: show.id }) + searchOf(show.playHref));
		},
		close() {
			sessions.close();
		}
	};

	function setSlot(el: HTMLElement | null) {
		slot = el;
	}
</script>

<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import { toastStore } from '$lib/toasts/store.svelte';
	import {
		MINI_MIN_SIZE,
		moveWithin,
		placeMini,
		resizeAtRatio,
		type Rect,
		type RememberedBounds
	} from './geometry';
	import MiniWindow from './MiniWindow.svelte';
	import MiniSwitcher from './MiniSwitcher.svelte';

	const BOUNDS_KEY = 'ani-gui:mini-player-mockups:bounds';

	let slotEl = $state<HTMLDivElement | undefined>();
	$effect(() => {
		setSlot(slotEl ?? null);
		return () => setSlot(null);
	});

	let rect = $state<Rect>({ x: 0, y: 0, width: 480, height: 270 });
	let area = $state<Rect>({ x: 0, y: 0, width: 1280, height: 720 });

	function readBounds(): RememberedBounds | null {
		try {
			const raw = browserStorage()?.getItem(BOUNDS_KEY);
			const v = raw ? (JSON.parse(raw) as RememberedBounds) : null;
			return v && typeof v.bounds?.width === 'number' ? v : null;
		} catch {
			return null;
		}
	}
	function writeBounds(r: Rect | null) {
		try {
			if (r) browserStorage()?.setItem(BOUNDS_KEY, JSON.stringify({ bounds: r, displayId: 0 }));
			else browserStorage()?.setItem(BOUNDS_KEY, 'null');
		} catch {
			// Not kept: this run still has it.
		}
	}

	function viewport(): Rect {
		return { x: 0, y: 0, width: window.innerWidth, height: window.innerHeight };
	}
	function ratioOf(video: HTMLVideoElement): number {
		return video.videoWidth && video.videoHeight ? video.videoWidth / video.videoHeight : 16 / 9;
	}

	// Where the window opens: the remembered size and, where the
	// platform allows, the remembered place; on Wayland the compositor
	// places it, which the mockup shows as the middle of the display.
	// It runs when a session opens or the platform changes, not on
	// every other preference.
	const platform = $derived(prefs.platform);
	$effect(() => {
		const s = session;
		if (!s) return;
		const display = viewport();
		area = display;
		const p = placeMini({
			remembered: readBounds(),
			displays: [{ id: 0, workArea: display }],
			mainDisplayId: 0,
			positionAllowed: platform !== 'wayland',
			ratio: ratioOf(s.video)
		});
		const pos = p.position ?? {
			x: Math.round((display.width - p.size.width) / 2),
			y: Math.round((display.height - p.size.height) / 2)
		};
		rect = { ...pos, ...p.size };
	});

	function track(e: PointerEvent, step: (dx: number, dy: number) => void) {
		if (e.button !== 0) return;
		const x0 = e.clientX;
		const y0 = e.clientY;
		const move = (ev: PointerEvent) => step(ev.clientX - x0, ev.clientY - y0);
		const up = () => {
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', up);
			writeBounds(rect);
		};
		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', up);
	}
	function onDragStart(e: PointerEvent) {
		const start = rect;
		track(e, (dx, dy) => (rect = moveWithin(start, dx, dy, area)));
	}
	function onResizeStart(e: PointerEvent) {
		e.stopPropagation();
		const start = rect;
		const s = session;
		if (!s) return;
		track(e, (dx, dy) => {
			const r = resizeAtRatio({
				start,
				dx,
				dy,
				ratio: ratioOf(s.video),
				min: MINI_MIN_SIZE,
				max: { width: area.width - start.x, height: area.height - start.y }
			});
			rect = r;
		});
	}

	// The "back to the app" option for a failure: the mini returns and
	// the play page shows its own error overlay, with a toast.
	$effect(() => {
		if (sim !== 'error' || prefs.failure !== 'hand-back') return;
		const fail = steps?.fail;
		miniMock.returnToApp();
		fail?.();
		toastStore.push({ kind: 'error', message: m.mini_handback_toast() });
	});

	$effect(() => {
		if (!session || prefs.platform !== 'wayland') {
			covered = false;
			return;
		}
		const onDown = (e: PointerEvent) => {
			const t = e.target as Element | null;
			if (t?.closest('.mini-sim, .mini-switcher, .mini-covered')) return;
			covered = true;
		};
		document.addEventListener('pointerdown', onDown, true);
		return () => document.removeEventListener('pointerdown', onDown, true);
	});
</script>

{#if prefs.enabled}
	<div
		class="mini-sim"
		class:mini-sim-on={session !== null}
		class:mini-sim-covered={covered}
		class:mini-sim-shadow={prefs.platform !== 'wayland'}
		style:inset-inline-start="{rect.x}px"
		style:inset-block-start="{rect.y}px"
		style:inline-size="{rect.width}px"
		style:block-size="{rect.height}px"
		aria-hidden={session === null}
	>
		<div class="mini-sim-video" bind:this={slotEl}></div>
		{#if session}
			{#key session}
				<MiniWindow
					{session}
					{prefs}
					{still}
					{steps}
					width={rect.width}
					height={rect.height}
					{sim}
					{skip}
					onReturn={() => miniMock.returnToApp()}
					onClose={() => miniMock.close()}
					{onDragStart}
					{onResizeStart}
					onClearSim={() => (sim = 'none')}
					onSkipDone={() => (skip = false)}
					onHintSeen={() => miniMock.setPrefs({ hintSeen: true })}
				/>
			{/key}
			<span class="mini-sim-tag" aria-hidden="true">{m.mini_window_tag()}</span>
		{/if}
	</div>
	{#if session && covered}
		<div
			class="mini-covered"
			style:inset-inline-start="{rect.x}px"
			style:inset-block-start="{rect.y}px"
			style:inline-size="{rect.width}px"
		>
			<span>{m.mini_covered_tag()}</span>
			<button type="button" onclick={() => (covered = false)}>{m.mini_covered_raise()}</button>
		</div>
	{/if}
	{#if prefs.switcher}
		<MiniSwitcher onForgetPlace={() => writeBounds(null)} />
	{/if}
{/if}

<style>
	.mini-sim {
		position: fixed;
		z-index: 900;
		display: none;
		background: #000;
		-webkit-app-region: no-drag;
		app-region: no-drag;
	}
	/* Windows and X11 window managers draw a shadow around a frameless
	   window; Electron 33's frameless window on Wayland has none. */
	.mini-sim-shadow {
		box-shadow: 0 10px 40px rgb(0 0 0 / 0.55);
	}
	.mini-sim-on {
		display: block;
	}
	/* Behind the main window: still playing, out of sight. */
	.mini-sim-covered {
		visibility: hidden;
	}
	.mini-covered {
		position: fixed;
		z-index: 900;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		padding: 4px 8px;
		border: 1px dashed var(--bone-400);
		border-radius: 4px;
		background: var(--ink-050);
		color: var(--bone-300);
		font: 500 0.75rem/1.4 var(--font-mono);
		-webkit-app-region: no-drag;
		app-region: no-drag;
	}
	.mini-covered button {
		padding: 2px 8px;
		border: 1px solid var(--ink-300);
		border-radius: var(--radius-control);
		background: transparent;
		color: var(--bone-100);
		font: inherit;
		cursor: pointer;
	}
	.mini-sim-video {
		position: absolute;
		inset: 0;
	}
	.mini-sim-video :global(video) {
		object-fit: contain;
	}
	.mini-sim-tag {
		position: absolute;
		inset-block-end: 100%;
		inset-inline-start: 0;
		margin-block-end: 4px;
		padding: 1px 6px;
		border: 1px dashed var(--bone-400);
		border-radius: 4px;
		background: var(--ink-050);
		color: var(--bone-300);
		font: 500 0.6875rem/1.4 var(--font-mono);
		pointer-events: none;
	}
</style>
