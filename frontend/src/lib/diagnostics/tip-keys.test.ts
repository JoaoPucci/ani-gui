import { describe, expect, it } from 'vitest';
import type { OnboardingDebugTip, OnboardingDebugView } from '$lib/api';
import { m } from '$lib/paraglide/messages';
import { asRunnable, installLabel, readTipKeys, windowLabel } from './tip-keys';

const tip = (over: Partial<OnboardingDebugTip> = {}): OnboardingDebugTip => ({
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
	seen: false,
	pending: true,
	places: ['watch-order-strip'],
	how_to: null,
	...over
});

describe('readTipKeys', () => {
	it('keeps the answer, or says the read failed', async () => {
		const view: OnboardingDebugView = { version: '0.19.0', installed_in: null, tips: [tip()] };
		expect(await readTipKeys(async () => view)).toEqual({ kind: 'read', view });
		expect(await readTipKeys(() => Promise.reject(new Error('down')))).toEqual({ kind: 'failed' });
	});
});

describe('windowLabel', () => {
	it('names the window the backend answered, inside it or not', () => {
		expect(windowLabel(tip())).toBe(
			m.diagnostics_tips_offered({ opens: '0.19.0', ends: '0.21.0' })
		);
		expect(windowLabel(tip({ offered: false }))).toBe(
			m.diagnostics_tips_outside({ opens: '0.19.0', ends: '0.21.0' })
		);
	});
	it('says when a tip runs only on demand, or waits on its flag', () => {
		expect(windowLabel(tip({ on_demand: true, opens: null, ends: null }))).toBe(
			m.diagnostics_tips_on_demand()
		);
		expect(windowLabel(tip({ flag: 'a_feature', flag_on: false, opens: null, ends: null }))).toBe(
			m.diagnostics_tips_flag_off({ flag: 'a_feature' })
		);
		expect(windowLabel(tip({ flag: 'a_feature', flag_on: true, opens: null, ends: null }))).toBe(
			m.diagnostics_tips_not_offered()
		);
	});
});

describe('installLabel', () => {
	it('says how the record began', () => {
		expect(installLabel({ version: '0.19.0', installed_in: null, tips: [] })).toBe(
			m.diagnostics_tips_updated()
		);
		expect(installLabel({ version: '0.19.0', installed_in: '0.19.0', tips: [] })).toBe(
			m.diagnostics_tips_installed_in({ version: '0.19.0' })
		);
	});
});

describe('asRunnable', () => {
	it('is the tip as the host runs it, pending', () => {
		expect(asRunnable(tip({ pending: false, seen: true }))).toEqual({
			id: 'watch-order',
			kind: 'single',
			surface: 'show',
			moment: 'arrive',
			since: '0.19.0',
			places: ['watch-order-strip'],
			how_to: null,
			pending: true
		});
	});
});
