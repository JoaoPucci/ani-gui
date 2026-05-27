<!--
  UpdateDialog — modal shown when the user clicks UpdateBadge.
  Surfaces the current and latest version tags + a primary action
  that opens the GitHub releases page in the system browser. The
  body field from the release (Markdown) is rendered as a
  pre-formatted code block — no Markdown parser dependency, just
  monospace text — so the user can skim the release notes without
  trying to interpret rendered HTML in-app.
-->
<script lang="ts">
	import { fly, fade } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { m } from '$lib/paraglide/messages';
	import { updateStore } from '$lib/update/store.svelte';

	interface Props {
		/** App version string from package.json — passed in so the
		 *  dialog can render "Current: 0.4.0" without re-reading the
		 *  package.json at runtime. */
		currentVersion: string;
	}
	let { currentVersion }: Props = $props();

	const open = $derived(updateStore.dialogOpen);
	const release = $derived(updateStore.available);

	function close() {
		updateStore.closeDialog();
	}

	function onBackdropPointerDown(event: PointerEvent) {
		// Only close on a click that started on the backdrop itself,
		// not on a drag that began inside the dialog and released
		// outside. Mirrors the player ErrorOverlay's pattern.
		if (event.target === event.currentTarget) close();
	}

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') close();
	}
</script>

<svelte:window onkeydown={open ? onKeydown : undefined} />

{#if open && release}
	<div
		class="update-backdrop"
		role="presentation"
		onpointerdown={onBackdropPointerDown}
		transition:fade={{ duration: 160 }}
	>
		<div
			class="update-dialog"
			role="dialog"
			aria-modal="true"
			aria-labelledby="update-dialog-title"
			transition:fly={{ y: 8, duration: 220, easing: cubicOut }}
		>
			<header class="update-dialog-head">
				<h2 id="update-dialog-title">{m.update_dialog_title()}</h2>
				<p>{m.update_dialog_intro()}</p>
			</header>

			<dl class="update-dialog-versions">
				<div>
					<dt>{m.update_dialog_current_label()}</dt>
					<dd>{currentVersion}</dd>
				</div>
				<div>
					<dt>{m.update_dialog_latest_label()}</dt>
					<dd class="latest">{release.tag}</dd>
				</div>
			</dl>

			{#if release.body}
				<pre class="update-dialog-body">{release.body}</pre>
			{/if}

			<footer class="update-dialog-foot">
				<button type="button" class="dlg-btn" onclick={close}>
					{m.update_dialog_close_label()}
				</button>
				<!-- External GitHub URL — goes through Electron's
				     setWindowOpenHandler → shell.openExternal, same
				     path as settings help links and ffmpeg.org from
				     the error overlay. resolve() is for in-app
				     SvelteKit routes; not applicable here. -->
				<!-- eslint-disable svelte/no-navigation-without-resolve -->
				<a
					class="dlg-btn dlg-btn-primary"
					href={release.url}
					target="_blank"
					rel="noreferrer noopener"
					aria-label={m.update_dialog_open_releases_aria_label()}
					onclick={close}
				>
					{m.update_dialog_open_releases_label()}
				</a>
				<!-- eslint-enable svelte/no-navigation-without-resolve -->
			</footer>
		</div>
	</div>
{/if}

<style>
	.update-backdrop {
		position: fixed;
		inset: 0;
		z-index: 1000;
		display: grid;
		place-items: center;
		padding: var(--space-5);
		background: color-mix(in oklab, var(--ink-000) 70%, transparent);
		backdrop-filter: blur(4px);
		-webkit-backdrop-filter: blur(4px);
	}
	.update-dialog {
		max-inline-size: 32rem;
		inline-size: 100%;
		max-block-size: min(36rem, 80dvh);
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		padding: var(--space-5);
		background: var(--ink-050);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-md);
		box-shadow:
			0 24px 48px -16px rgb(0 0 0 / 0.55),
			0 0 0 1px color-mix(in oklab, var(--brand) 14%, transparent);
		color: var(--bone-100);
		font-family: var(--font-body);
	}
	.update-dialog-head h2 {
		margin: 0 0 var(--space-2);
		font-family: var(--font-display, var(--font-body));
		font-size: var(--type-display-s, 1.5rem);
		letter-spacing: var(--tracking-display);
	}
	.update-dialog-head p {
		margin: 0;
		color: var(--bone-200);
		font-size: var(--type-body-l);
	}
	.update-dialog-versions {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-3);
		margin: 0;
	}
	.update-dialog-versions > div {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		padding: var(--space-3);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-sm);
	}
	.update-dialog-versions dt {
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		letter-spacing: var(--tracking-micro);
		text-transform: uppercase;
		color: var(--bone-300);
	}
	.update-dialog-versions dd {
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--type-body-l);
		color: var(--bone-100);
	}
	.update-dialog-versions dd.latest {
		color: var(--brand);
		font-weight: 600;
	}
	.update-dialog-body {
		margin: 0;
		padding: var(--space-3);
		background: color-mix(in oklab, var(--ink-000) 50%, transparent);
		border: 1px solid var(--ink-200);
		border-radius: var(--radius-sm);
		font-family: var(--font-mono);
		font-size: var(--type-meta);
		line-height: 1.55;
		color: var(--bone-200);
		max-block-size: 18rem;
		overflow: auto;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.update-dialog-foot {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}
	.dlg-btn {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-sm);
		font-family: var(--font-body);
		font-size: var(--type-body-s);
		font-weight: 500;
		border: 1px solid var(--ink-200);
		background: var(--ink-100);
		color: var(--bone-100);
		text-decoration: none;
		cursor: pointer;
		transition:
			background var(--dur-fast) var(--ease-out-soft),
			border-color var(--dur-fast) var(--ease-out-soft);
	}
	.dlg-btn:hover {
		background: color-mix(in oklab, var(--bone-100) 6%, var(--ink-100));
	}
	.dlg-btn-primary {
		background: var(--brand);
		border-color: var(--brand);
		color: var(--bone-100);
	}
	.dlg-btn-primary:hover {
		background: color-mix(in oklab, var(--brand) 80%, var(--bone-100));
	}
	.dlg-btn:focus-visible {
		outline: 2px solid var(--brand);
		outline-offset: 2px;
	}
</style>
