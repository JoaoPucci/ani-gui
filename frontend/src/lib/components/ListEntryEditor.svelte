<!--
  ListEntryEditor — detail-page control for the show's tracker list entry.

  Sits in the detail action row beside Play/Download. The trigger shows
  the live status ("Watching · 5/24") or "Add to list" when the show
  isn't tracked yet; clicking opens an anchored popover with a status
  dropdown + an episode stepper + Save / Remove. Writes fan out to every
  connected tracker (set-entry.ts); the editor opens on the LIVE current
  entry passed in, so the user never edits from a stale snapshot.

  Explicit edits are authoritative — the backend writes the typed value
  verbatim (no monotonic guard), so a downward episode correction goes
  through. Logic lives in tested helpers (list-entry-view, set-entry);
  this component is the thin view + wiring.
-->
<script lang="ts">
	import { createPopoverControls } from '$lib/account/popover-controls';
	import { syncRemoveEntry, syncSetEntry } from '$lib/account/set-entry';
	import {
		editorToggle,
		runEditorRemove,
		runEditorSave,
		settleRemove,
		settleSave
	} from '$lib/account/editor-actions';
	import { deriveListEntryView, listButtonLabel } from '$lib/account/list-entry-view';
	import {
		seedForOpen,
		pendingAfterSave,
		statusWriteIntended
	} from '$lib/account/list-entry-pending';
	import { clampProgress, effectiveProgress, statusOptionsFor } from '$lib/account/list-entry-edit';
	import type { EntryView, ListStatus } from '$lib/account/types';
	import { onDestroy } from 'svelte';
	import { reopenForm, type Flight } from '$lib/account/editor-flight';
	import { editorFlights } from '$lib/account/editor-flights.svelte';
	import { adoptedLanding, stillHere, type Watched } from '$lib/account/editor-lifecycle';
	import { listAccounts } from '$lib/account/held-account';
	import { accountStore } from '$lib/account/store.svelte';
	import { toastStore } from '$lib/toasts/store.svelte';
	import { m } from '$lib/paraglide/messages';

	let {
		kitsuId,
		total = null,
		cap = null,
		current = null,
		airing = false,
		disabled = false,
		notice = null,
		onReconcile = () => {}
	}: {
		kitsuId: string;
		/** Announced episode total — the display denominator ("· 5/24") and
		 *  the count a Completed entry snaps to. */
		total?: number | null;
		/** Highest episode you can set — the last aired/streamable one. You
		 *  can't mark episodes that aren't out yet. Defaults to `total`. */
		cap?: number | null;
		current?: EntryView | null;
		/** The show hasn't finished airing — Completed/Rewatching are hidden. */
		airing?: boolean;
		disabled?: boolean;
		/** Why the control is disabled, when there is something to say — the
		 *  tracker's rate limit and its wait. Shown beside the trigger. */
		notice?: string | null;
		/** Re-read the live entry from the trackers after a write settles. The
		 *  button is optimistic for instant feedback, but this reconcile is the
		 *  source of truth — it overwrites the optimistic value with what the
		 *  trackers actually hold (and auto-retries through rate-limits), so the
		 *  UI can't stay out of sync after a partial/failed write. */
		onReconcile?: () => void;
	} = $props();

	// The episode the stepper/input can't exceed: the last aired one when
	// known, else the announced total.
	const settableCap = $derived(cap ?? total);

	// The live entry we display. A writable `$derived` so it tracks the
	// `current` prop (which the page loads + updates async) yet can be
	// overwritten optimistically after a Save/Remove until the prop
	// re-syncs.
	let live = $derived(current);

	const view = $derived(deriveListEntryView(live, total));

	// While the show is airing, Completed/Rewatching are hidden — unless the
	// entry is already set to one (keep it visible so we don't downgrade it).
	const statusChoices = $derived(statusOptionsFor(airing, view.status));

	function statusLabel(s: ListStatus): string {
		switch (s) {
			case 'planning':
				return m.detail_list_status_planning();
			case 'watching':
				return m.detail_list_status_watching();
			case 'completed':
				return m.detail_list_status_completed();
			case 'paused':
				return m.detail_list_status_paused();
			case 'dropped':
				return m.detail_list_status_dropped();
			case 'rewatching':
				return m.detail_list_status_rewatching();
		}
	}

	const buttonLabel = $derived(listButtonLabel(view, { add: m.detail_list_add(), statusLabel }));

	let open = $state(false);
	// The kitsuId the open popover was seeded for. When the detail route swaps
	// shows (it reuses this component), this lets us close a form that's now
	// stale against a different kitsuId — see the auto-close effect.
	let openedFor = $state<string | null>(null);
	// Writes in flight, per show, in a store that outlives this instance: the
	// route reuses this editor across shows and unmounts it whenever it leaves
	// one, while a save can wait out a tracker's rate limit for minutes. A save
	// still out for a show locks that show's editor — this instance or one
	// mounted later — and no other, until it lands or its trackers' accounts
	// change.
	const flight = $derived(editorFlights.locking(kitsuId));
	const saving = $derived(flight?.kind === 'saving');
	const removing = $derived(flight?.kind === 'removing');
	let trigger = $state<HTMLButtonElement | null>(null);
	let editStatus = $state<ListStatus>('planning');
	let editProgress = $state(0);
	// The status the editor opened on (the seed). A save sends status only when
	// the user moved it off this, so a progress-only edit doesn't converge a
	// divergent status across trackers.
	let seededStatus = $state<ListStatus>('planning');
	// A partial save's unfinished business: the pre-save seed + the status the
	// user wanted, kept per-show so a retry still re-sends the status to the
	// tracker that failed even after the popover is dismissed and reopened (on
	// reopen the live view has moved to the landed value, which would otherwise
	// make the status look unchanged). Cleared on a clean save/remove. See
	// list-entry-pending.ts.
	// Kept in the same store as the flights, so it survives a remount too.
	const pendingEdit = $derived(editorFlights.pendingFor(kitsuId));
	// True while a partial-save retry is unresolved for the open show: it forces
	// the next Save to write status to every tracker (so re-picking the original
	// status reverts the one that moved, and a plain retry re-sends the intent).
	const pendingActive = $derived(pendingEdit !== null);

	// This instance, for the writes it starts; and whether it is gone, so a
	// write that settles after it unmounts updates the store but not it.
	const self = Symbol('list editor');
	let destroyed = false;
	onDestroy(() => {
		destroyed = true;
	});

	// A flight another instance started — one since unmounted — that ends on
	// the show on screen leaves that instance's page to re-read nothing: this
	// one re-reads its own. Watched on the flight itself, not on whether it
	// still locks: an account change lifting the lock is no landing.
	let watched: Watched = { kitsuId: '', flight: null };
	const watchedFlight = $derived(editorFlights.flightOf(kitsuId));
	$effect(() => {
		const next: Watched = { kitsuId, flight: watchedFlight };
		const landing = adoptedLanding(watched, next, self, open);
		watched = next;
		// The popover shows the sent form, which the re-read entry replaces.
		if (landing.close) open = false;
		if (landing.reconcile) onReconcile();
	});

	// ✕ / click-outside dismiss without writing — Save is the only commit, so
	// adding (which opens on the default Plan to Watch) needs an explicit click.
	// Allowed while a write is out (it can wait out a tracker's rate limit for
	// a while): the write carries on with what it captured when it started,
	// and its toast reports how it ended.
	function closeEditor() {
		open = false;
	}

	// Blocking, NOT optimistic: the popover shows "Saving…" with the controls
	// disabled until the write confirms, so the button never shows a value that
	// isn't really on a tracker and the user can't pile up edits while a result
	// is pending. The user may dismiss it meanwhile (a save can wait out a
	// tracker's rate limit); reopened, it shows the save still out, locked. On
	// success we apply the confirmed value, toast, close, and reconcile
	// (multi-tracker truth). On failure we toast and leave the popover as it is
	// so the edit can be retried.
	async function save() {
		if (disabled || flight) return;
		const id = kitsuId;
		const statusChanged = statusWriteIntended(pendingActive, editStatus, seededStatus);
		// What this save sends, captured now: the popover can be dismissed while
		// the save waits, and the form is not what it sent by the time it lands.
		const sentStatus = editStatus;
		const sentProgress = editProgress;
		const started: Flight = {
			kind: 'saving',
			sent: { status: sentStatus, progress: sentProgress, seededStatus },
			owner: self,
			listed: listAccounts(accountStore.connected)
		};
		editorFlights.begin(id, started);
		const res = await runEditorSave(
			{ syncSetEntry },
			{
				kitsuId: id,
				disabled,
				save: { status: sentStatus, statusChanged, progress: sentProgress, total }
			}
		);
		editorFlights.end(id, started);
		// The pending retry is the show's, in the store: kept whether or not this
		// instance is still here to show it.
		const outcome = res.kind === 'superseded' ? 'noop' : res.kind;
		editorFlights.setPending(
			id,
			pendingAfterSave(
				editorFlights.pendingFor(id),
				id,
				outcome,
				statusChanged,
				sentStatus,
				sentProgress
			),
			started.listed
		);
		// The toast always; this instance's own state only while it is alive and
		// on the show the save was for.
		const settled = settleSave(res, stillHere({ destroyed, kitsuId }, id));
		if (settled.toast) toastStore.push(settled.toast);
		if (!settled.local || (res.kind !== 'partial' && res.kind !== 'saved')) return;
		// Some trackers took it (partial) or all did: apply the landed value to
		// the button now, even if the reconcile read is delayed or rate-limited.
		// A partial keeps the popover open so an in-place retry re-sends the
		// status to the trackers that failed; the pending retry carries that
		// intent across a dismiss/reopen too.
		live = res.live;
		if (res.kind === 'saved') open = false;
		onReconcile();
	}

	const popoverControls = createPopoverControls({
		getTrigger: () => trigger,
		getPopoverId: () => 'list-entry-pop'
	});
	// Attach the outside-click/Escape listener ONCE for the component's life and
	// gate the close on `open` inside the handler. Re-attaching per open (an
	// `$effect` that reads `open`) raced on a fast close→reopen — signal
	// coalescing could skip the re-run, leaving no listener so the next outside
	// click didn't dismiss.
	$effect(() => {
		return popoverControls.attach({
			onClose: () => {
				if (open) closeEditor();
			}
		});
	});

	// Close a stale form. The detail route reuses this component across shows,
	// so an open popover must not survive a navigation and stay Save-able against
	// the new kitsuId.
	$effect(() => {
		// (1) The show actually changed (navigation): close even mid-write — the
		// user left this title, so any in-flight save (guarded by its own kitsuId
		// snapshot) won't touch the new show, and the abandoned retry is moot.
		if (open && kitsuId !== openedFor) {
			open = false;
			return;
		}
		// (2) Same show, transiently disabled: a save that refreshes a near-expiry
		// token flips accountStore and kicks a loud re-read that briefly disables
		// us. Don't close mid-write on that account — the user is watching the
		// save. Once the write settles this re-runs and closes if needed.
		if (disabled && !saving && !removing) open = false;
	});

	function toggle() {
		const act = editorToggle({ open, busy: saving || removing, disabled });
		if (act === 'ignore') return;
		if (act === 'close') {
			closeEditor();
			return;
		}
		const sent = act === 'reopen' && flight ? reopenForm(flight) : null;
		if (sent) {
			// The save still out for this show: show what it sent, locked.
			editStatus = sent.status;
			editProgress = sent.progress;
			if (sent.seededStatus) seededStatus = sent.seededStatus;
		} else {
			// Re-seed from a surviving pending edit (an unfinished partial-save
			// retry for this show) when present, else from the live view.
			const seed = seedForOpen(view, pendingEdit, kitsuId);
			editStatus = seed.status;
			editProgress = seed.progress;
			seededStatus = seed.seededStatus;
		}
		openedFor = kitsuId;
		open = true;
	}

	// Completed always means the full count, so the episode field is locked to
	// the total while Completed is selected (and editing is disabled below).
	const episodeLocked = $derived(editStatus === 'completed' && total !== null);
	// Stepper bounds: + stops at the last aired/settable episode, − at 0, so
	// the disabled button is the feedback that you've hit the limit.
	const atCap = $derived(settableCap !== null && editProgress >= settableCap);
	const atFloor = $derived(editProgress <= 0);
	// A write is in flight — lock the whole popover until it confirms.
	const busy = $derived(saving || removing);

	function pickStatus(s: ListStatus) {
		editStatus = s;
		// Completed snaps to the full count and Planning zeroes — but a plain
		// status change does NOT clamp the existing count to the settable cap: a
		// tracker can legitimately sit above the currently-streamable cap (e.g.
		// 20/24 watched while dub exposes 12), and clamping here would silently
		// lower it. Only user episode edits (step/onProgressInput) clamp.
		editProgress = effectiveProgress(s, editProgress, total);
	}

	function step(delta: number) {
		editProgress = effectiveProgress(
			editStatus,
			clampProgress(editProgress + delta, settableCap),
			total
		);
	}

	function onProgressInput(e: Event) {
		editProgress = effectiveProgress(
			editStatus,
			clampProgress(Number.parseInt((e.currentTarget as HTMLInputElement).value, 10), settableCap),
			total
		);
	}

	// Blocking, same shape as save(): "Removing…" with controls disabled until
	// the delete confirms; success closes + toasts + reconciles; failure keeps
	// the popover open with an error to retry.
	async function remove() {
		if (disabled || flight) return;
		const id = kitsuId;
		const started: Flight = {
			kind: 'removing',
			owner: self,
			listed: listAccounts(accountStore.connected)
		};
		editorFlights.begin(id, started);
		const res = await runEditorRemove({ syncRemoveEntry }, { kitsuId: id, disabled });
		editorFlights.end(id, started);
		// The entry's gone everywhere, so any pending status retry is moot.
		if (res.kind === 'removed') editorFlights.setPending(id, null);
		const settled = settleRemove(res, stillHere({ destroyed, kitsuId }, id));
		if (settled.toast) toastStore.push(settled.toast);
		// A failure keeps the popover open to retry; a show changed mid-remove,
		// or an editor since unmounted, leaves the state on screen alone.
		if (!settled.local) return;
		live = null;
		open = false;
		onReconcile();
	}
</script>

<div class="list-entry">
	<button
		bind:this={trigger}
		type="button"
		class="le-trigger"
		class:on-list={view.onList}
		aria-haspopup="dialog"
		aria-expanded={open}
		{disabled}
		onclick={toggle}
	>
		<span aria-hidden="true">{view.onList ? '✓' : '＋'}</span>
		<span>{buttonLabel}</span>
	</button>
	{#if notice}
		<p class="le-notice" role="status">{notice}</p>
	{/if}

	{#if open}
		<div id="list-entry-pop" class="le-pop" role="dialog" aria-label={m.detail_list_editor_aria()}>
			<header class="le-head">
				<span class="le-title">{m.detail_list_editor_aria()}</span>
				<button
					type="button"
					class="le-close"
					aria-label={m.detail_list_close()}
					onclick={closeEditor}>✕</button
				>
			</header>

			<label class="le-field">
				<span class="le-label">{m.detail_list_status_label()}</span>
				<div class="le-select-wrap">
					<select
						class="le-select"
						value={editStatus}
						disabled={busy}
						onchange={(e) => pickStatus((e.currentTarget as HTMLSelectElement).value as ListStatus)}
					>
						{#each statusChoices as s (s)}
							<option value={s}>{statusLabel(s)}</option>
						{/each}
					</select>
					<span class="le-caret" aria-hidden="true">▾</span>
				</div>
			</label>

			<!-- Plan to Watch means not started, so there's no episode to set. -->
			{#if editStatus !== 'planning'}
				<div class="le-field">
					<span class="le-label">{m.detail_list_episode_label()}</span>
					<div class="le-stepper">
						<button
							type="button"
							class="le-step"
							aria-label={m.detail_list_episode_decrement()}
							disabled={episodeLocked || atFloor || busy}
							onclick={() => step(-1)}>−</button
						>
						<input
							class="le-count"
							type="number"
							min="0"
							max={settableCap ?? undefined}
							inputmode="numeric"
							value={editProgress}
							disabled={episodeLocked || busy}
							oninput={onProgressInput}
							aria-label={m.detail_list_episode_label()}
						/>
						<button
							type="button"
							class="le-step"
							aria-label={m.detail_list_episode_increment()}
							disabled={episodeLocked || atCap || busy}
							onclick={() => step(1)}>+</button
						>
						{#if total !== null}
							<span class="le-total">/ {total}</span>
						{/if}
					</div>
				</div>
			{/if}

			<footer class="le-foot">
				{#if view.onList}
					<button type="button" class="le-remove" disabled={disabled || busy} onclick={remove}>
						{removing ? m.detail_list_removing() : m.detail_list_remove()}
					</button>
				{/if}
				<button type="button" class="le-save" disabled={disabled || busy} onclick={save}>
					{saving ? m.detail_list_saving() : m.detail_list_save()}
				</button>
			</footer>
		</div>
	{/if}
</div>

<style>
	.list-entry {
		position: relative;
		display: inline-flex;
		align-items: center;
		gap: var(--space-3);
	}
	/* The reason a disabled trigger is disabled, in the row's meta voice:
	   quiet, one line, no box of its own. */
	.le-notice {
		margin: 0;
		/* A flex item keeps its content width unless told otherwise; a
		   long sentence would widen the row rather than wrap. */
		min-inline-size: 0;
		max-inline-size: 36ch;
		font-size: var(--type-meta);
		color: var(--bone-300);
		text-wrap: balance;
	}
	/* Matches the detail action row's .btn-outline: frosted outline,
	   mono meta type, uppercase tracking. */
	.le-trigger {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding-block: var(--space-3);
		padding-inline: var(--space-5);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-100);
		background: transparent;
		border: 1px solid var(--bone-300);
		border-radius: var(--radius-control, 6px);
		cursor: pointer;
		transition:
			border-color var(--dur-fast) var(--ease-out-soft),
			color var(--dur-fast) var(--ease-out-soft);
		white-space: nowrap;
	}
	.le-trigger:hover:not(:disabled) {
		border-color: var(--bone-100);
	}
	.le-trigger:disabled {
		opacity: 0.5;
		/* not-allowed, not progress: a disabled trigger (entry still loading or a
		   read that errored out) shouldn't read as an indefinite spinner. */
		cursor: not-allowed;
	}
	.le-trigger.on-list {
		border-color: color-mix(in oklab, var(--accent-jade) 55%, var(--bone-300));
		color: var(--bone-100);
	}

	.le-pop {
		position: absolute;
		inset-block-start: calc(100% + var(--space-2));
		inset-inline-start: 0;
		min-inline-size: 16rem;
		display: grid;
		gap: var(--space-3);
		padding: var(--space-4);
		background: var(--ink-050, #15120f);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-card, 8px);
		box-shadow: var(--shadow-card-hover, 0 14px 40px rgba(0, 0, 0, 0.45));
		z-index: 60;
	}
	.le-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}
	.le-title {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.le-close {
		display: grid;
		place-items: center;
		inline-size: 1.5rem;
		block-size: 1.5rem;
		margin-inline-end: calc(-1 * var(--space-1));
		font-size: 0.9rem;
		line-height: 1;
		color: var(--bone-400);
		background: transparent;
		border: 0;
		border-radius: var(--radius-control, 6px);
		cursor: pointer;
	}
	.le-close:hover {
		color: var(--bone-100);
		background: var(--ink-200);
	}
	.le-field {
		display: grid;
		gap: var(--space-1);
	}
	.le-label {
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-400);
	}
	.le-select-wrap {
		position: relative;
		display: block;
	}
	.le-select {
		/* Drop the native arrow and draw our own caret (.le-caret) so it
		   sits with breathing room from the edge, not jammed against it.
		   inline-end padding reserves room for the caret. */
		appearance: none;
		inline-size: 100%;
		padding: var(--space-2) calc(var(--space-3) + 1.25rem) var(--space-2) var(--space-3);
		font: inherit;
		color: var(--bone-100);
		background: var(--ink-000);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-control, 6px);
	}
	.le-caret {
		position: absolute;
		inset-block: 0;
		inset-inline-end: var(--space-3);
		display: grid;
		place-items: center;
		font-family: var(--font-mono);
		color: var(--bone-300);
		pointer-events: none;
	}
	.le-stepper {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
	.le-step {
		inline-size: 2rem;
		block-size: 2rem;
		font-size: 1.1rem;
		line-height: 1;
		color: var(--bone-100);
		background: var(--ink-000);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-control, 6px);
		cursor: pointer;
	}
	.le-step:hover:not(:disabled) {
		border-color: var(--bone-300);
	}
	.le-step:disabled,
	.le-count:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
	.le-count {
		inline-size: 4rem;
		padding: var(--space-2);
		font: inherit;
		text-align: center;
		color: var(--bone-100);
		background: var(--ink-000);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-control, 6px);
		font-variant-numeric: tabular-nums;
		/* The custom −/+ steppers replace the native spinners, which would
		   otherwise double up the controls. */
		appearance: textfield;
		-moz-appearance: textfield;
	}
	.le-count::-webkit-outer-spin-button,
	.le-count::-webkit-inner-spin-button {
		-webkit-appearance: none;
		margin: 0;
	}
	.le-total {
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		color: var(--bone-400);
	}
	/* Footer: destructive Remove on the left, Save (primary) on the right. */
	.le-foot {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin-block-start: var(--space-1);
	}
	.le-remove {
		padding: 0;
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--accent-oxblood);
		background: transparent;
		border: 0;
		cursor: pointer;
	}
	.le-remove:hover:not(:disabled) {
		text-decoration: underline;
	}
	.le-save {
		margin-inline-start: auto;
		padding: var(--space-2) var(--space-4);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--ink-000);
		background: var(--accent, var(--brand));
		border: 1px solid transparent;
		border-radius: var(--radius-control, 6px);
		cursor: pointer;
	}
	.le-remove:disabled,
	.le-save:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
