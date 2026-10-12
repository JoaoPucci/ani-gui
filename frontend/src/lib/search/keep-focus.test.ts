import { describe, expect, it, vi } from 'vitest';
import { keepFocus, type FocusSource } from './keep-focus';

class FakeControl implements FocusSource {
	isConnected = true;
	private listeners = new Map<string, () => void>();
	addEventListener(type: string, l: () => void) {
		this.listeners.set(type, l);
	}
	removeEventListener(type: string) {
		this.listeners.delete(type);
	}
	emit(type: 'focus' | 'blur') {
		this.listeners.get(type)?.();
	}
}

const flush = () => new Promise<void>((r) => queueMicrotask(r));

describe('keepFocus', () => {
	it('hands focus to the target, without scrolling, when a focused control leaves', async () => {
		const control = new FakeControl();
		const target = { focus: vi.fn() };
		const action = keepFocus(control, () => target);
		control.emit('focus');
		// Removal can blur the control on its way out; that does not
		// count as the reader moving focus elsewhere.
		control.isConnected = false;
		control.emit('blur');
		await flush();
		action.destroy();
		expect(target.focus).toHaveBeenCalledWith({ preventScroll: true });
	});

	it('leaves focus alone when the control never held it', () => {
		const target = { focus: vi.fn() };
		keepFocus(new FakeControl(), () => target).destroy();
		expect(target.focus).not.toHaveBeenCalled();
	});

	it('leaves focus alone when the reader had already moved it away', async () => {
		const control = new FakeControl();
		const target = { focus: vi.fn() };
		const action = keepFocus(control, () => target);
		control.emit('focus');
		control.emit('blur');
		await flush();
		action.destroy();
		expect(target.focus).not.toHaveBeenCalled();
	});

	it('copes with a target that is already gone', () => {
		const control = new FakeControl();
		const action = keepFocus(control, () => null);
		control.emit('focus');
		expect(() => action.destroy()).not.toThrow();
	});
});
