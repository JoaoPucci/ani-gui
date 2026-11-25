import { describe, expect, it, vi } from 'vitest';
import type { OnboardingDebugTip, OnboardingDebugView } from '$lib/api';
import { TipSeenToggles } from './tip-seen-toggle.svelte';

const tip = (seen: boolean): OnboardingDebugTip => ({
	id: 'watch-order',
	kind: 'single',
	surface: 'show',
	moment: 'arrive',
	since: '0.19.0',
	until: null,
	opens: '0.19.0',
	ends: '0.21.0',
	flag: null,
	flag_on: false,
	flag_on_since: null,
	on_demand: false,
	offered: true,
	seen,
	pending: !seen,
	places: ['watch-order-strip'],
	how_to: null
});
const view = (seen: boolean): OnboardingDebugView => ({
	version: '0.19.0',
	installed_in: null,
	tips: [tip(seen)]
});

describe('TipSeenToggles', () => {
	it('reads the list', async () => {
		const t = new TipSeenToggles({ send: vi.fn(), read: async () => view(false), tell: vi.fn() });
		await t.load();
		expect(t.state).toEqual({ kind: 'read', view: view(false) });
	});

	it('sends a change, tells the session, and reads the list again', async () => {
		const send = vi.fn(async () => {});
		const tell = vi.fn();
		let seen = false;
		const t = new TipSeenToggles({ send, read: async () => view(seen), tell });
		await t.load();
		seen = true;
		await t.set(tip(false), { checked: true });
		expect(send).toHaveBeenCalledWith('watch-order', true);
		expect(tell).toHaveBeenCalledWith('watch-order', true);
		expect(t.state).toEqual({ kind: 'read', view: view(true) });
		expect(t.failed).toBe(false);
		expect(t.busy).toBe(false);
	});

	it('a refused change goes back on the box, tells nobody, and says so', async () => {
		const tell = vi.fn();
		const t = new TipSeenToggles({
			send: () => Promise.reject(new Error('503')),
			read: async () => view(false),
			tell
		});
		const box = { checked: true };
		await t.set(tip(false), box);
		expect(box.checked).toBe(false);
		expect(tell).not.toHaveBeenCalled();
		expect(t.failed).toBe(true);
	});

	it('keeps the last list when the read after a change fails, and says so', async () => {
		let reads = 0;
		const t = new TipSeenToggles({
			send: async () => {},
			read: async () => {
				reads += 1;
				if (reads > 1) throw new Error('down');
				return view(false);
			},
			tell: vi.fn()
		});
		await t.load();
		await t.set(tip(false), { checked: true });
		expect(t.state).toEqual({ kind: 'read', view: view(false) });
		expect(t.failed).toBe(true);
	});

	it('holds every toggle while one change is out, and ignores a second', async () => {
		let release!: () => void;
		const send = vi.fn(() => new Promise<void>((r) => (release = r)));
		const t = new TipSeenToggles({ send, read: async () => view(true), tell: vi.fn() });
		const first = t.set(tip(false), { checked: true });
		expect(t.busy).toBe(true);
		await t.set(tip(false), { checked: false });
		expect(send).toHaveBeenCalledTimes(1);
		release();
		await first;
		expect(t.busy).toBe(false);
	});
});
