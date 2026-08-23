// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createPopoverControls } from './popover-controls';

describe('createPopoverControls', () => {
	let trigger: HTMLButtonElement;
	let popover: HTMLDivElement;
	let outside: HTMLDivElement;

	beforeEach(() => {
		trigger = document.createElement('button');
		popover = document.createElement('div');
		popover.id = 'pop';
		outside = document.createElement('div');
		document.body.append(trigger, popover, outside);
	});

	afterEach(() => {
		document.body.replaceChildren();
	});

	it('returns a detach function from attach', () => {
		const controls = createPopoverControls({
			getTrigger: () => trigger,
			getPopoverId: () => 'pop'
		});
		const detach = controls.attach({ onClose: vi.fn() });
		expect(typeof detach).toBe('function');
		detach();
	});

	it('calls onClose when a pointerdown lands outside both trigger and popover', () => {
		const onClose = vi.fn();
		const detach = createPopoverControls({
			getTrigger: () => trigger,
			getPopoverId: () => 'pop'
		}).attach({ onClose });
		outside.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
		expect(onClose).toHaveBeenCalledTimes(1);
		detach();
	});

	it('does NOT close when a pointerdown lands on the trigger', () => {
		const onClose = vi.fn();
		const detach = createPopoverControls({
			getTrigger: () => trigger,
			getPopoverId: () => 'pop'
		}).attach({ onClose });
		trigger.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
		expect(onClose).not.toHaveBeenCalled();
		detach();
	});

	it('does NOT close when a pointerdown lands inside the popover', () => {
		const inner = document.createElement('span');
		popover.append(inner);
		const onClose = vi.fn();
		const detach = createPopoverControls({
			getTrigger: () => trigger,
			getPopoverId: () => 'pop'
		}).attach({ onClose });
		inner.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
		expect(onClose).not.toHaveBeenCalled();
		detach();
	});

	it('closes on Escape', () => {
		const onClose = vi.fn();
		const detach = createPopoverControls({
			getTrigger: () => trigger,
			getPopoverId: () => 'pop'
		}).attach({ onClose });
		document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		expect(onClose).toHaveBeenCalledTimes(1);
		detach();
	});

	it('ignores non-Escape keydowns', () => {
		const onClose = vi.fn();
		const detach = createPopoverControls({
			getTrigger: () => trigger,
			getPopoverId: () => 'pop'
		}).attach({ onClose });
		document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
		document.dispatchEvent(new KeyboardEvent('keydown', { key: 'a' }));
		expect(onClose).not.toHaveBeenCalled();
		detach();
	});

	it('stops firing after detach()', () => {
		const onClose = vi.fn();
		const detach = createPopoverControls({
			getTrigger: () => trigger,
			getPopoverId: () => 'pop'
		}).attach({ onClose });
		detach();
		outside.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
		document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
		expect(onClose).not.toHaveBeenCalled();
	});

	it('survives a pointerdown when getTrigger returns null (trigger unmounted mid-effect)', () => {
		const onClose = vi.fn();
		const detach = createPopoverControls({
			getTrigger: () => null,
			getPopoverId: () => 'pop'
		}).attach({ onClose });
		// No trigger → outside-click guard treats every event as outside.
		outside.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
		expect(onClose).toHaveBeenCalledTimes(1);
		detach();
	});
});
