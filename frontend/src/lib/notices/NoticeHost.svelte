<!--
  The notification center's wiring, mounted by the layout only while
  the center's flag is on. It draws nothing: it loads the stored
  events, routes the center's toasts to the app's toast stack, feeds
  it what the window is doing, and turns the update check, the download
  list and the source's health into notices. The rules it calls are in
  the sibling modules; this is the adapter.
-->
<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { sourceHealth } from '$lib/api';
	import { downloadStore } from '$lib/download/store.svelte';
	import { toastStore } from '$lib/toasts/store.svelte';
	import { updateStore } from '$lib/update/store.svelte';
	import { noticeCenter, setNoticeToast } from './instance';
	import { watchMoment } from './moment';
	import { startHealthPoll } from './health-poll';
	import { noticeToast, viewOf } from './panel';
	import { downloadChanges, outageChanges, updateChange } from './sources';

	let { onPlayer, appVersion }: { onPlayer: boolean; appVersion: string } = $props();

	const seenDownloads = new Set<string>();

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
		const stopMoment = watchMoment(document, (at) => noticeCenter.setMoment(at));
		const poll = startHealthPoll({
			read: sourceHealth,
			onHealth: (h) => noticeCenter.take(outageChanges(h)),
			setInterval: (fn, ms) => setInterval(() => fn(), ms),
			clearInterval: (h) => clearInterval(h as ReturnType<typeof setInterval>)
		});
		const onFocus = () => poll.poke();
		window.addEventListener('focus', onFocus);
		return () => {
			window.removeEventListener('focus', onFocus);
			poll.stop();
			stopMoment();
			setNoticeToast(() => {});
		};
	});

	// Each effect tracks its source only; applying the change reads and
	// writes the center's list, which must not re-run the effect.
	$effect(() => {
		const change = updateChange(updateStore.available?.tag ?? null, appVersion);
		untrack(() => noticeCenter.take([change]));
	});

	$effect(() => {
		const changes = downloadChanges(downloadStore.items, seenDownloads);
		untrack(() => noticeCenter.take(changes));
	});
</script>
