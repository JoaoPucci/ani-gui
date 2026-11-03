<!--
  The notification center's wiring, mounted by the layout only while
  the center's flag is on. It draws nothing: it loads the stored
  events, routes the center's toasts to the app's toast stack, feeds
  it what the window is doing, and turns the update check, the download
  list, the source's health and what the maintainer published (read
  once, from the backend) into notices, and the published number into
  the donation ask. The rules it calls are in the sibling modules; this
  is the adapter.
-->
<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { publishedNotices, sourceHealth } from '$lib/api';
	import { m } from '$lib/paraglide/messages';
	import { downloadStore } from '$lib/download/store.svelte';
	import { toastStore } from '$lib/toasts/store.svelte';
	import { updateStore } from '$lib/update/store.svelte';
	import {
		browserStorage,
		noticeCenter,
		setMomentReader,
		setNoticeObserver,
		setNoticeToast
	} from './instance';
	import { createDonationAsk } from './donation-ask';
	import {
		parsePublished,
		publishedChanges,
		sourceNote,
		withSourceNote,
		type SourceNote
	} from './published';
	import {
		PUBLISHED_STORAGE_KEY,
		keepPublished,
		parseMemory,
		remember,
		serializeMemory,
		type PublishedMemory
	} from './published-memory';
	import type { Moment } from './arrival';
	import type { Notice } from './model';
	import { momentOf, watchMoment } from './moment';
	import { startHealthPoll } from './health-poll';
	import { noticeToast, viewOf } from './panel';
	import { downloadChanges, outageChanges, updateChange, type ProviderHealth } from './sources';

	let { onPlayer, appVersion }: { onPlayer: boolean; appVersion: string } = $props();

	const seenDownloads = new Set<string>();
	const storage = browserStorage(window);
	let note: SourceNote | null = null;
	let health: ProviderHealth[] | null = null;
	let moment: Moment = { playing: false, fullscreen: false };

	function readMemory(): PublishedMemory {
		try {
			return parseMemory(storage?.getItem(PUBLISHED_STORAGE_KEY) ?? null);
		} catch {
			return parseMemory(null);
		}
	}

	let memory = readMemory();

	function store(next: PublishedMemory) {
		if (serializeMemory(next) === serializeMemory(memory)) return;
		memory = next;
		try {
			storage?.setItem(PUBLISHED_STORAGE_KEY, serializeMemory(next));
		} catch {
			// Refused: read and dismissed hold for this session only.
		}
	}

	function keep(before: readonly Notice[], after: readonly Notice[]) {
		store(remember(memory, before, after));
	}

	// Never over a video, fullscreen or the player: the ask waits, and
	// one already up goes when any of them begins (it was acted on, so it
	// does not come back).
	let askToast: string | null = null;
	const ask = createDonationAsk({
		storage,
		now: () => Date.now(),
		show: () => {
			askToast = toastStore.push({
				kind: 'info',
				message: m.notices_donation_ask(),
				duration: null,
				actionLabel: m.notices_donation_donate(),
				// eslint-disable-next-line svelte/no-navigation-without-resolve -- resolve builds the path; the hash names the About page's section
				onAction: () => void goto(`${resolve('/about')}#support`),
				dismissLabel: m.notices_donation_later(),
				onClose: () => (askToast = null)
			});
		}
	});
	const busy = () => moment.playing || moment.fullscreen || onPlayer;

	function setBusy(now: boolean) {
		if (now && askToast !== null) toastStore.dismiss(askToast);
		ask.setBusy(now);
	}

	function view(key: string) {
		const v = viewOf(key, onPlayer);
		noticeCenter.markRead(key);
		if (v.mode === 'in-place') {
			noticeCenter.expanded = v.key;
			noticeCenter.open = true;
		} else {
			// eslint-disable-next-line svelte/no-navigation-without-resolve -- viewOf builds the route's own path
			void goto(v.path);
		}
	}

	onMount(() => {
		noticeCenter.load();
		setNoticeToast((n) => toastStore.push(noticeToast(n, view)));
		setMomentReader(() => momentOf(document));
		setNoticeObserver(keep);
		const stopMoment = watchMoment(document, (at) => {
			noticeCenter.setMoment(at);
			moment = at;
			setBusy(busy());
		});
		let alive = true;
		void publishedNotices()
			.then(parsePublished, () => parsePublished(null))
			.then((p) => {
				if (!alive) return;
				note = sourceNote(p.items);
				store(
					keepPublished(
						memory,
						p.items.map((i) => i.id)
					)
				);
				noticeCenter.take(publishedChanges(p.items, memory));
				ask.setNumber(p.donationAsk);
				// An outage row raised before the answer gets its note now.
				if (health) noticeCenter.take(withSourceNote(outageChanges(health), note));
			});
		const poll = startHealthPoll({
			read: sourceHealth,
			onHealth: (h) => {
				health = h;
				noticeCenter.take(withSourceNote(outageChanges(h), note));
			},
			setInterval: (fn, ms) => setInterval(() => fn(), ms),
			clearInterval: (h) => clearInterval(h as ReturnType<typeof setInterval>)
		});
		const onFocus = () => poll.poke();
		window.addEventListener('focus', onFocus);
		return () => {
			alive = false;
			setNoticeObserver(null);
			window.removeEventListener('focus', onFocus);
			poll.stop();
			stopMoment();
			setNoticeToast(() => {});
			setMomentReader(null);
		};
	});

	// Each effect tracks its source only; applying the change reads and
	// writes the center's list, which must not re-run the effect.
	$effect(() => {
		const player = onPlayer;
		untrack(() => setBusy(player || moment.playing || moment.fullscreen));
	});

	$effect(() => {
		const change = updateChange(updateStore.available?.tag ?? null, appVersion);
		untrack(() => noticeCenter.take([change]));
	});

	$effect(() => {
		const changes = downloadChanges(downloadStore.items, seenDownloads);
		untrack(() => noticeCenter.take(changes));
	});
</script>
