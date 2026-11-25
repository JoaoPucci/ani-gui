<!--
  The feature tips' host, mounted once at the layout root. It asks the
  backend which tips are pending ($./tips.svelte), watches the places
  this page's pending tips point at ($./watch), and when one has
  settled on screen ($./gate) shows it as a spotlight: the page dims
  around the place ($./Scrim.svelte) and a balloon explains it
  ($./Balloon.svelte). One tip per page visit, never in the player,
  over a dialog or in fullscreen. $./spotlight-session holds the tip on
  screen and how it closes.
-->
<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { page } from '$app/state';
	import { m } from '$lib/paraglide/messages';
	import { isModalOpen, watchModals } from '$lib/components/modal-layer';
	import { scrollBehavior } from '$lib/motion/scroll';
	import Balloon from './Balloon.svelte';
	import Scrim from './Scrim.svelte';
	import { arrivesOn, arrivingNow, surfaceOf, watchedHere } from './gate';
	import { armedNow } from './on-demand';
	import { placeBalloon } from './place';
	import { litArea, passHoles } from './spotlight';
	import { SpotlightSession } from './spotlight-session.svelte';
	import { onboarding } from './tips.svelte';
	import { PlaceWatcher, browserWatchDeps, placeElement } from './watch';
	import { foundSteps } from './tip-steps.svelte';
	import { balloonActions, runAction } from './balloon-actions';
	import { stepWords } from './words';

	let settled = $state<ReadonlySet<string>>(new Set());
	let fullscreen = $state(false);
	let modalOpen = $state(false);
	let closedThisVisit = $state(false);
	let session = $state<SpotlightSession | null>(null);
	let size = $state<{ w: number; h: number } | null>(null);
	let balloon = $state<HTMLDivElement | undefined>();
	let scrim = $state<HTMLDivElement | undefined>();

	const surface = $derived(surfaceOf(page.route?.id ?? null));
	const watched = $derived([
		...new Set([
			...watchedHere(onboarding.tips, surface).map((t) => t.places[0]),
			...(onboarding.armed && arrivesOn({ ...onboarding.armed, pending: true }, surface)
				? onboarding.armed.places.slice(0, 1)
				: [])
		])
	]);
	const words = $derived(session ? stepWords(session.tip.id, session.steps.number) : null);
	/** How the tip on screen shows: as offered, as a Debug preview, or as
	 *  a walkthrough asked for ("How it works", no version). */
	let shownAs = $state<'tip' | 'preview' | 'guide'>('tip');
	const lit = $derived(session ? litArea(session.rect, session.view) : null);
	const placement = $derived(session && lit && size ? placeBalloon(lit, size, session.view) : null);

	onMount(() => {
		void onboarding.load();
		// A dialog or fullscreen already up when the host mounts holds
		// the tips back too.
		modalOpen = isModalOpen();
		fullscreen = Boolean(document.fullscreenElement);
		const onFs = () => {
			fullscreen = Boolean(document.fullscreenElement);
			if (fullscreen) session?.close('yield');
		};
		const stopModals = watchModals(() => {
			modalOpen = isModalOpen();
			session?.modalsChanged();
		});
		let frame = 0;
		// Scrolls and resizes move the place: read it again, once a frame
		// at most, only while a tip is on screen.
		const reread = () => {
			if (!session || frame) return;
			frame = requestAnimationFrame(() => {
				frame = 0;
				session?.read();
			});
		};
		document.addEventListener('fullscreenchange', onFs);
		window.addEventListener('scroll', reread, { capture: true, passive: true });
		window.addEventListener('resize', reread);
		return () => {
			stopModals();
			cancelAnimationFrame(frame);
			document.removeEventListener('fullscreenchange', onFs);
			window.removeEventListener('scroll', reread, { capture: true });
			window.removeEventListener('resize', reread);
		};
	});

	// Watch the places this page's pending tips point at.
	$effect(() => {
		const places = watched;
		if (places.length === 0) return;
		const w = new PlaceWatcher(places, (s) => (settled = new Set(s)), browserWatchDeps());
		return () => {
			w.stop();
			settled = new Set();
		};
	});

	// A new page visit may show a tip again. A tip on screen when the
	// page changes was not closed, so it yields, still pending.
	$effect(() => {
		void page.url.pathname;
		untrack(() => {
			session?.close('yield');
			closedThisVisit = false;
			onboarding.pageChanged(surface, page.url.pathname);
		});
	});

	// Show a tip asked for on demand, else the pending tip whose place
	// has settled, when none is up.
	$effect(() => {
		if (session) return;
		const screen = { surface, fullscreen, modalOpen, settled: (p: string) => settled.has(p) };
		const asked = armedNow(onboarding.armed, screen);
		const tip = asked ?? arrivingNow(onboarding.tips, { ...screen, closedThisVisit });
		if (!tip || !stepWords(tip.id, 1)) return;
		const found = foundSteps(tip.places, (p) => placeElement(document, p));
		if (found[0]?.n !== 1) return;
		untrack(() => {
			const askedAs = onboarding.armedAs;
			shownAs = asked ? askedAs : 'tip';
			if (asked) onboarding.disarm();
			size = null;
			session = new SpotlightSession(
				tip,
				found.map((f) => f.el),
				{
					doc: document,
					now: () => performance.now(),
					// A tip shown on demand (a preview, or a walkthrough asked
					// for) marks nothing seen, and counts as the visit's one tip,
					// so a pending tip does not follow it on the same visit.
					markSeen: (id) => {
						if (!asked) onboarding.markSeen(id);
					},
					onClosed: (seen) => {
						if (seen) closedThisVisit = true;
						// A tip asked for that gave way is asked for again, as
						// it was, so it never comes back as the real tip.
						else if (asked) onboarding.rearm(tip, askedAs);
						session = null;
					}
				},
				found.map((f) => f.n)
			);
		});
	});

	// Once placed, the balloon holds the page as its topmost modal
	// (the session scrolls a place with no room into room first).
	$effect(() => {
		const s = session;
		if (!s || !placement || !balloon || !size) return;
		s.placed(placement, size, balloon, scrim ?? null, (el, margin) => {
			el.style.scrollMarginTop = `${margin}px`;
			el.scrollIntoView({ block: 'start', behavior: scrollBehavior() });
			el.style.scrollMarginTop = '';
		});
	});

	// A press on the lit place works, and closes the tip.
	function onPointer(e: PointerEvent) {
		session?.pressed(e.target);
	}
</script>

<svelte:document onpointerdowncapture={onPointer} />

{#if session && words && lit}
	<Scrim
		bind:root={scrim}
		{lit}
		place={session.rect}
		passHoles={passHoles(session.buttons, lit)}
		view={session.view}
		accent={session.accent}
		travelling={session.steps.travelling}
		step={session.steps.seq.index}
		onArrived={() => session?.steps.arrived()}
		onOutside={() => session?.close('outside')}
	/>
	<Balloon
		bind:root={balloon}
		title={words.title()}
		body={words.body()}
		version={shownAs === 'guide' ? '' : session.tip.since}
		newLabel={shownAs === 'guide' ? m.onboarding_guide() : m.onboarding_new()}
		{placement}
		accent={session.accent}
		compact={session.view.h < 700}
		actions={balloonActions(session.tip, session.steps.seq)}
		step={session.steps.seq.total > 1
			? { index: session.steps.seq.index + 1, total: session.steps.seq.total }
			: null}
		stepLabel={m.onboarding_step_aria({
			n: String(session.steps.seq.index + 1),
			total: String(session.steps.seq.total)
		})}
		onAction={(id) => session && runAction(id, session, onboarding)}
		measure={(s) => (size = s)}
	/>
{/if}
