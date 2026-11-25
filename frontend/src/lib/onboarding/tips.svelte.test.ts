import { describe, expect, it, vi } from 'vitest';
import type { OnboardingTip, OnboardingView } from '$lib/api';
import { OnboardingTips, __resetOnboardingForTests, onboarding } from './tips.svelte';
import { onboardingSeen, onboardingTips } from '$lib/api';

vi.mock('$lib/api', () => ({ onboardingTips: vi.fn(), onboardingSeen: vi.fn() }));

const tip = (id: string, pending = true): OnboardingTip => ({
	id,
	kind: 'single',
	surface: 'show',
	moment: 'arrive',
	since: '0.19.0',
	places: [`${id}-place`],
	how_to: null,
	pending
});
const VIEW: OnboardingView = { version: '0.19.0', tips: [tip('a'), tip('b', false)] };

describe('OnboardingTips', () => {
	it('asks the backend once and keeps its answer', async () => {
		const fetchTips = vi.fn(async () => VIEW);
		const store = new OnboardingTips(
			fetchTips,
			vi.fn(async () => {})
		);
		expect(store.tips).toEqual([]);
		await Promise.all([store.load(), store.load()]);
		expect(fetchTips).toHaveBeenCalledTimes(1);
		expect(store.tips.map((t) => t.id)).toEqual(['a', 'b']);
	});

	it('has no tips when the read fails', async () => {
		const store = new OnboardingTips(
			() => Promise.reject(new Error('down')),
			vi.fn(async () => {})
		);
		await store.load();
		expect(store.tips).toEqual([]);
	});

	it('marks a closed tip seen here at once and tells the backend', async () => {
		const sendSeen = vi.fn(async () => {});
		const store = new OnboardingTips(async () => VIEW, sendSeen);
		await store.load();
		store.markSeen('a');
		expect(store.tips.find((t) => t.id === 'a')?.pending).toBe(false);
		expect(sendSeen).toHaveBeenCalledWith('a');
	});

	it('keeps a close the backend refused for the session', async () => {
		const store = new OnboardingTips(
			async () => VIEW,
			() => Promise.reject(new Error('io'))
		);
		await store.load();
		store.markSeen('a');
		await Promise.resolve();
		expect(store.tips.find((t) => t.id === 'a')?.pending).toBe(false);
	});

	it('arms a tip asked for on demand until it shows', () => {
		const store = new OnboardingTips(
			async () => VIEW,
			vi.fn(async () => {})
		);
		store.showNow(tip('a', false));
		expect(store.armed?.id).toBe('a');
		store.disarm();
		expect(store.armed).toBeNull();
	});

	it('the session’s store reads the backend’s routes, and a reset asks again', async () => {
		const read = vi.mocked(onboardingTips).mockResolvedValue(VIEW);
		const send = vi.mocked(onboardingSeen).mockResolvedValue(undefined);
		__resetOnboardingForTests();
		await onboarding.load();
		expect(onboarding.tips).toHaveLength(2);
		onboarding.markSeen('a');
		expect(send).toHaveBeenCalledWith('a');
		__resetOnboardingForTests();
		expect(onboarding.tips).toEqual([]);
		await onboarding.load();
		expect(read).toHaveBeenCalledTimes(2);
	});
});
