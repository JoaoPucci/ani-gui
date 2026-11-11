import { describe, expect, it, vi } from 'vitest';
import { createDonationAsk } from './donation-ask';
import { DONATION_STORAGE_KEY, GRACE_MS, serializeState } from './donation';

const T0 = 1_800_000_000_000;
const DAY = 24 * 3600_000;

function storage(initial: string | null = null) {
	let value = initial;
	return {
		getItem: (k: string) => (k === DONATION_STORAGE_KEY ? value : null),
		setItem: (k: string, v: string) => {
			if (k === DONATION_STORAGE_KEY) value = v;
		},
		read: () => value
	};
}

/** An install first launched at T0 that saw the number 2 then. */
const installed = () =>
	storage(serializeState({ v: 1, firstLaunchAt: T0, baseline: 2, acted: null, optOut: false }));

function ask(store: ReturnType<typeof storage>, now = T0 + GRACE_MS + DAY) {
	const show = vi.fn();
	const a = createDonationAsk({ storage: store, now: () => now, show });
	return { a, show };
}

describe('the donation ask, wired', () => {
	it('records a new install with the number it sees, and asks nothing', () => {
		const s = storage();
		const { a, show } = ask(s, T0);
		a.setBusy(false);
		a.setNumber(2);
		expect(show).not.toHaveBeenCalled();
		expect(JSON.parse(s.read() as string)).toMatchObject({ firstLaunchAt: T0, baseline: 2 });
	});

	it('asks once when the number rises, after the grace period, and remembers it asked', () => {
		const s = installed();
		const { a, show } = ask(s);
		a.setBusy(false);
		a.setNumber(3);
		expect(show).toHaveBeenCalledTimes(1);
		a.setBusy(true);
		a.setBusy(false);
		expect(show).toHaveBeenCalledTimes(1);
		expect(JSON.parse(s.read() as string).acted).toBe(3);
		const again = ask(s);
		again.a.setBusy(false);
		again.a.setNumber(3);
		expect(again.show).not.toHaveBeenCalled();
	});

	it('waits until the number is known and nothing is playing', () => {
		const { a, show } = ask(installed());
		a.setBusy(true);
		a.setNumber(3);
		expect(show).not.toHaveBeenCalled();
		a.setBusy(false);
		expect(show).toHaveBeenCalledTimes(1);
	});

	it('asks nothing until it knows whether something is playing', () => {
		const { a, show } = ask(installed());
		a.setNumber(3);
		expect(show).not.toHaveBeenCalled();
	});

	it('asks nothing within the grace period', () => {
		const { a, show } = ask(installed(), T0 + GRACE_MS - 1);
		a.setBusy(false);
		a.setNumber(3);
		expect(show).not.toHaveBeenCalled();
	});

	it('works on without storage, and with storage that refuses', () => {
		const refusing = {
			getItem: () => {
				throw new Error('no');
			},
			setItem: () => {
				throw new Error('no');
			}
		};
		const show = vi.fn();
		const a = createDonationAsk({ storage: refusing, now: () => T0, show });
		a.setBusy(false);
		expect(() => a.setNumber(2)).not.toThrow();
		const b = createDonationAsk({ storage: null, now: () => T0, show });
		b.setBusy(false);
		expect(() => b.setNumber(2)).not.toThrow();
		expect(show).not.toHaveBeenCalled();
	});
});
