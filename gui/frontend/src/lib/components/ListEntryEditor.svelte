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
		STATUS_OPTIONS,
		deriveListEntryView,
		editorInitial,
		listButtonLabel
	} from '$lib/account/list-entry-view';
	import type { EntryView, ListStatus } from '$lib/account/types';
	import { toastStore } from '$lib/toasts/store.svelte';
	import { m } from '$lib/paraglide/messages';

	let {
		kitsuId,
		total = null,
		current = null,
		loading = false
	}: {
		kitsuId: string;
		total?: number | null;
		current?: EntryView | null;
		loading?: boolean;
	} = $props();

	// The live entry we display. A writable `$derived` so it tracks the
	// `current` prop (which the page loads + updates async) yet can be
	// overwritten optimistically after a Save/Remove until the prop
	// re-syncs.
	let live = $derived(current);

	const view = $derived(deriveListEntryView(live, total));

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

	const popoverControls = createPopoverControls({
		getTrigger: () => trigger,
		getPopoverId: () => 'list-entry-pop'
	});
	$effect(() => {
		if (!open) return;
		return popoverControls.attach({ onClose: () => (open = false) });
	});

	function toggle() {
		// Don't open until the live entry has settled — otherwise the editor
		// would seed Planning/0 and a Save could overwrite a real entry that
		// just hadn't arrived yet.
		if (loading) return;
		if (open) {
			open = false;
			return;
		}
		const init = editorInitial(view);
		editStatus = init.status;
		editProgress = init.progress;
		open = true;
	}

	function pickStatus(s: ListStatus) {
		editStatus = s;
		// Picking Completed pre-fills the episode count to the total — the
		// common intent. The user can still adjust it before saving.
		if (s === 'completed' && total !== null) editProgress = total;
	}

	function step(delta: number) {
		const next = editProgress + delta;
		editProgress = next < 0 ? 0 : next;
	}

	function onProgressInput(e: Event) {
		const raw = Number.parseInt((e.currentTarget as HTMLInputElement).value, 10);
		editProgress = Number.isFinite(raw) && raw > 0 ? raw : 0;
	}

	async function save() {
		busy = true;
		try {
			const n = await syncSetEntry(kitsuId, { status: editStatus, progress: editProgress });
			if (n > 0) {
				live = { status: editStatus, progress: editProgress };
				toastStore.push({ kind: 'success', message: m.detail_list_saved() });
				open = false;
			} else {
				toastStore.push({ kind: 'error', message: m.detail_list_save_failed() });
			}
		} catch {
			toastStore.push({ kind: 'error', message: m.detail_list_save_failed() });
		} finally {
			busy = false;
		}
	}

	async function remove() {
		busy = true;
		try {
			const n = await syncRemoveEntry(kitsuId);
			// The fan-out swallows per-provider failures and returns the
			// success count, so mirror the save path: only clear local state
			// and report success when at least one tracker actually removed
			// the entry. n === 0 (offline/401/no bearer) is a failure — the
			// entry is still on the tracker, so don't pretend it's gone.
			if (n > 0) {
				live = null;
				toastStore.push({ kind: 'success', message: m.detail_list_removed() });
				open = false;
			} else {
				toastStore.push({ kind: 'error', message: m.detail_list_save_failed() });
			}
		} catch {
			toastStore.push({ kind: 'error', message: m.detail_list_save_failed() });
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
		disabled={loading}
		onclick={toggle}
	>
		<span aria-hidden="true">{view.onList ? '✓' : '＋'}</span>
		<span>{buttonLabel}</span>
	</button>

	{#if open}
		<div id="list-entry-pop" class="le-pop" role="dialog" aria-label={m.detail_list_editor_aria()}>
			<label class="le-field">
				<span class="le-label">{m.detail_list_status_label()}</span>
				<select
					class="le-select"
					value={editStatus}
					onchange={(e) => pickStatus((e.currentTarget as HTMLSelectElement).value as ListStatus)}
				>
					{#each STATUS_OPTIONS as s (s)}
						<option value={s}>{statusLabel(s)}</option>
					{/each}
				</select>
			</label>

			<div class="le-field">
				<span class="le-label">{m.detail_list_episode_label()}</span>
				<div class="le-stepper">
					<button
						type="button"
						class="le-step"
						aria-label={m.detail_list_episode_decrement()}
						onclick={() => step(-1)}>−</button
					>
					<input
						class="le-count"
						type="number"
						min="0"
						inputmode="numeric"
						value={editProgress}
						oninput={onProgressInput}
						aria-label={m.detail_list_episode_label()}
					/>
					<button
						type="button"
						class="le-step"
						aria-label={m.detail_list_episode_increment()}
						onclick={() => step(1)}>+</button
					>
					{#if total !== null}
						<span class="le-total">/ {total}</span>
					{/if}
				</div>
			</div>

			<div class="le-actions">
				{#if view.onList}
					<button type="button" class="le-remove" disabled={busy} onclick={remove}>
						{m.detail_list_remove()}
					</button>
				{/if}
				<button type="button" class="le-save" disabled={busy} onclick={save}>
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
	.le-select {
		padding: var(--space-2) var(--space-3);
		font: inherit;
		color: var(--bone-100);
		background: var(--ink-000);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-control, 6px);
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
	.le-step:hover {
		border-color: var(--bone-300);
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
