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
	import { startPublishedRead } from './published-read';
	import { createPublishedMemoryStore } from './published-memory-store';
	import type { Moment } from './arrival';
	import { momentOf, watchMoment } from './moment';
	import { startHealthPoll } from './health-poll';
	import { noticeToast, viewOf } from './panel';
	import { downloadChanges, updateChange } from './sources';
	import { createOutageNote } from './outage-note';
	import { createAskToast } from './ask-toast';

	let { onPlayer, appVersion }: { onPlayer: boolean; appVersion: string } = $props();

	const seenDownloads = new Set<string>();
	const storage = browserStorage(window);
	const outageNote = createOutageNote();
	let moment: Moment = { playing: false, fullscreen: false };

	const memory = createPublishedMemoryStore(storage);

	// Never over a video, fullscreen or the player: the ask waits, and
	// one already up goes when any of them begins (it was acted on, so it
	// does not come back).
	const askToast = createAskToast(toastStore, () => ({
		kind: 'info',
		message: m.notices_donation_ask(),
		duration: null,
		actionLabel: m.notices_donation_donate(),
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- resolve builds the path; the hash names the About page's section
		onAction: () => void goto(`${resolve('/about')}#support`),
		dismissLabel: m.notices_donation_later()
	}));
	const ask = createDonationAsk({ storage, now: () => Date.now(), show: askToast.show });
	const busyWith = (player: boolean) => player || moment.playing || moment.fullscreen;

	function setBusy(now: boolean) {
		askToast.busy(now);
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
		setNoticeObserver(memory.observe);
		const stopMoment = watchMoment(document, (at) => {
			noticeCenter.setMoment(at);
			moment = at;
			setBusy(busyWith(onPlayer));
		});
		const stopRead = startPublishedRead({
			read: publishedNotices,
			keepPublished: memory.keepPublished,
			joinNote: outageNote.published,
			take: (c) => noticeCenter.take(c),
			setNumber: (n) => ask.setNumber(n)
		});
		const poll = startHealthPoll({
			read: sourceHealth,
			onHealth: (h) => noticeCenter.take(outageNote.health(h)),
			setInterval: (fn, ms) => setInterval(() => fn(), ms),
			clearInterval: (h) => clearInterval(h as ReturnType<typeof setInterval>)
		});
		const onFocus = () => poll.poke();
		window.addEventListener('focus', onFocus);
		return () => {
			stopRead();
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
		untrack(() => setBusy(busyWith(player)));
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
