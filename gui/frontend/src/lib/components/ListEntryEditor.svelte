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
	import { runEditorRemove, runEditorSave } from '$lib/account/editor-actions';
	import {
		deriveListEntryView,
		editorInitial,
		listButtonLabel
	} from '$lib/account/list-entry-view';
	import { clampProgress, effectiveProgress, statusOptionsFor } from '$lib/account/list-entry-edit';
	import type { EntryView, ListStatus } from '$lib/account/types';
	import { toastStore } from '$lib/toasts/store.svelte';
	import { m } from '$lib/paraglide/messages';

	let {
		kitsuId,
		total = null,
		cap = null,
		current = null,
		airing = false,
		disabled = false
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
	let busy = $state(false);
	let trigger = $state<HTMLButtonElement | null>(null);
	let editStatus = $state<ListStatus>('planning');
	let editProgress = $state(0);
	// The status the editor opened on (the seed). Save sends status only when
	// the user moved it off this, so a save that only touched the episode
	// count doesn't converge a divergent status across trackers.
	let seededStatus = $state<ListStatus>('planning');

	const popoverControls = createPopoverControls({
		getTrigger: () => trigger,
		getPopoverId: () => 'list-entry-pop'
	});
	$effect(() => {
		if (!open) return;
		return popoverControls.attach({ onClose: () => (open = false) });
	});

	// Close the editor whenever it becomes disabled while open. The detail
	// route reuses this component across shows, so navigating with the popover
	// open would otherwise leave the previous show's form values visible — and
	// Save-able — against the new kitsuId before its live tracker state is
	// known. Closing forces a reopen, which reseeds from the new show's view.
	$effect(() => {
		if (disabled) open = false;
	});

	function toggle() {
		// Don't open while disabled — the live entry is still loading or its
		// read failed, so the form would seed Planning/0 and a Save could
		// overwrite a real entry whose status we don't actually know yet.
		if (disabled) return;
		if (open) {
			open = false;
			return;
		}
		const init = editorInitial(view);
		editStatus = init.status;
		editProgress = init.progress;
		seededStatus = init.status;
		open = true;
	}

	// Completed always means the full count, so the episode field is locked to
	// the total while Completed is selected (and editing is disabled below).
	const episodeLocked = $derived(editStatus === 'completed' && total !== null);
	// Stepper bounds: + stops at the last aired/settable episode, − at 0, so
	// the disabled button is the feedback that you've hit the limit.
	const atCap = $derived(settableCap !== null && editProgress >= settableCap);
	const atFloor = $derived(editProgress <= 0);

	function pickStatus(s: ListStatus) {
		editStatus = s;
		// Completed snaps the episode count to the total — you can't be
		// completed with fewer (status wins over a partial count).
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

	async function save() {
		busy = true;
		// Snapshot the form values before awaiting. The status/episode controls
		// stay editable during the in-flight save, so reading editStatus/
		// editProgress again after the await could optimistically show values
		// the user changed mid-request that were never sent to the tracker.
		const save = { status: editStatus, seededStatus, progress: editProgress, total };
		try {
			// runEditorSave owns the state machine (disabled gate, per-tracker
			// fan-out outcome interpretation, optimistic `live`); the component
			// just applies the result + picks the toast. See editor-actions.ts.
			const res = await runEditorSave({ syncSetEntry }, { kitsuId, disabled, save });
			if (res.kind === 'saved') {
				live = res.live;
				toastStore.push({ kind: 'success', message: m.detail_list_saved() });
				open = false;
			} else if (res.kind === 'failed') {
				toastStore.push({ kind: 'error', message: m.detail_list_save_failed() });
			}
		} finally {
			busy = false;
		}
	}

	async function remove() {
		busy = true;
		try {
			const res = await runEditorRemove({ syncRemoveEntry }, { kitsuId, disabled });
			if (res.kind === 'removed') {
				live = null;
				toastStore.push({ kind: 'success', message: m.detail_list_removed() });
				open = false;
			} else if (res.kind === 'failed') {
				toastStore.push({ kind: 'error', message: m.detail_list_save_failed() });
			}
		} finally {
			busy = false;
		}
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

	{#if open}
		<div id="list-entry-pop" class="le-pop" role="dialog" aria-label={m.detail_list_editor_aria()}>
			<label class="le-field">
				<span class="le-label">{m.detail_list_status_label()}</span>
				<div class="le-select-wrap">
					<select
						class="le-select"
						value={editStatus}
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
							disabled={episodeLocked || atFloor}
							onclick={() => step(-1)}>−</button
						>
						<input
							class="le-count"
							type="number"
							min="0"
							max={settableCap ?? undefined}
							inputmode="numeric"
							value={editProgress}
							disabled={episodeLocked}
							oninput={onProgressInput}
							aria-label={m.detail_list_episode_label()}
						/>
						<button
							type="button"
							class="le-step"
							aria-label={m.detail_list_episode_increment()}
							disabled={episodeLocked || atCap}
							onclick={() => step(1)}>+</button
						>
						{#if total !== null}
							<span class="le-total">/ {total}</span>
						{/if}
					</div>
				</div>
			{/if}

			<div class="le-actions">
				{#if view.onList}
					<button type="button" class="le-remove" disabled={busy || disabled} onclick={remove}>
						{m.detail_list_remove()}
					</button>
				{/if}
				<button type="button" class="le-save" disabled={busy || disabled} onclick={save}>
					{m.detail_list_save()}
				</button>
			</div>
		</div>
	{/if}
</div>

<style>
	.list-entry {
		position: relative;
		display: inline-flex;
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
		letter-spacing: var(--tracking-wide);
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
		cursor: progress;
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
	.le-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
		margin-block-start: var(--space-1);
	}
	.le-save,
	.le-remove {
		padding: var(--space-2) var(--space-4);
		font-family: var(--font-mono);
		font-size: var(--type-micro);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		border-radius: var(--radius-control, 6px);
		cursor: pointer;
	}
	.le-save {
		color: var(--ink-000);
		background: var(--accent, var(--brand));
		border: 1px solid transparent;
	}
	.le-remove {
		color: var(--accent-oxblood);
		background: transparent;
		border: 1px solid color-mix(in oklab, var(--accent-oxblood) 50%, transparent);
	}
	.le-save:disabled,
	.le-remove:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
