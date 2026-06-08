/**
 * Headless popover state machine — outside-click + Escape close.
 *
 * Lives in `$lib` (not inside the .svelte) so the menu-closing
 * behavior gets unit coverage, per AGENTS.md §2: Svelte components
 * stay as thin adapters; imperative DOM listeners belong in a
 * sibling TypeScript module with tests. Used by AccountChip;
 * future popovers can reuse it via the same `getTrigger` /
 * `getPopoverId` injection.
 */

export interface PopoverDeps {
	/** Returns the trigger element, or null if it isn't mounted yet
	 *  (Svelte `bind:this` is null until the first render commits). */
	getTrigger(): HTMLElement | null;
	/** The DOM id of the popover panel. Looked up at event time so
	 *  the helper survives the popover being mounted lazily. */
	getPopoverId(): string;
}

export interface PopoverAttachOptions {
	onClose(): void;
}

export interface PopoverControls {
	/** Attach the listeners. Returns a detach function — call it when
	 *  the popover closes or the parent component unmounts. */
	attach(opts: PopoverAttachOptions): () => void;
}

export function createPopoverControls(deps: PopoverDeps): PopoverControls {
	return {
		attach({ onClose }) {
			const onPointerDown = (e: PointerEvent) => {
				const target = e.target as Node | null;
				if (!target) return;
				const trigger = deps.getTrigger();
				if (trigger?.contains(target)) return;
				const pop = document.getElementById(deps.getPopoverId());
				if (pop?.contains(target)) return;
				onClose();
			};
			const onKey = (e: KeyboardEvent) => {
				if (e.key === 'Escape') onClose();
			};
			document.addEventListener('pointerdown', onPointerDown);
			document.addEventListener('keydown', onKey);
			return () => {
				document.removeEventListener('pointerdown', onPointerDown);
				document.removeEventListener('keydown', onKey);
			};
		}
	};
}
