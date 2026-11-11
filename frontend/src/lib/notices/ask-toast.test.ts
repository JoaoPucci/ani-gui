import { describe, expect, it, vi } from 'vitest';
import { createAskToast } from './ask-toast';
import type { PushArgs } from '$lib/toasts/store.svelte';

function toasts() {
	const closers = new Map<string, () => void>();
	let n = 0;
	const port = {
		push: vi.fn((a: PushArgs) => {
			const id = `t${++n}`;
			closers.set(id, () => a.onClose?.());
			return id;
		}),
		dismiss: vi.fn((id: string) => {
			closers.get(id)?.();
			closers.delete(id);
		})
	};
	return { port, closeByUser: (id: string) => port.dismiss(id) };
}
const ARGS: PushArgs = { kind: 'info', message: 'ask', duration: null };

describe('the donation toast while something else takes the screen', () => {
	it('goes when the window becomes busy while it is up, once', () => {
		const { port } = toasts();
		const toast = createAskToast(port, () => ARGS);
		toast.show();
		expect(port.push).toHaveBeenCalledWith(expect.objectContaining({ message: 'ask' }));
		toast.busy(true);
		toast.busy(true);
		expect(port.dismiss).toHaveBeenCalledTimes(1);
		expect(port.dismiss).toHaveBeenCalledWith('t1');
	});

	it('dismisses nothing when it is not up, has already gone, or the window is idle', () => {
		const t = toasts();
		const toast = createAskToast(t.port, () => ARGS);
		toast.busy(true);
		toast.show();
		toast.busy(false);
		expect(t.port.dismiss).not.toHaveBeenCalled();
		t.closeByUser('t1');
		toast.busy(true);
		expect(t.port.dismiss).toHaveBeenCalledTimes(1);
	});

	it("still calls the toast's own close handler", () => {
		const { port } = toasts();
		const onClose = vi.fn();
		const toast = createAskToast(port, () => ({ ...ARGS, onClose }));
		toast.show();
		toast.busy(true);
		expect(onClose).toHaveBeenCalledTimes(1);
	});
});
