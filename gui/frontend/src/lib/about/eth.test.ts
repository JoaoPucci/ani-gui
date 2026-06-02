/**
 * ETH-address shape guard for the About page donation block.
 *
 * The address itself is hard-coded in the credits module; this guard
 * is a sanity check a future edit might trip if someone fat-fingers
 * the constant. Strictly: 0x + exactly 40 hex chars, no case
 * constraint (EIP-55 checksum casing is a separate concern we don't
 * enforce on display).
 *
 * (The page originally also surfaced a `buildMetamaskSendUrl` helper
 * that produced a metamask.app.link target. That link is a mobile
 * universal-link only — on desktop it opens a useless landing page
 * in the system browser since MetaMask Extension intentionally has
 * no compose-send deep link. Dropped in favour of copy-to-clipboard
 * which works everywhere.)
 */
import { describe, it, expect } from 'vitest';
import { isValidEthAddress } from './eth';

describe('isValidEthAddress', () => {
	it('accepts a canonical 0x-prefixed 40-hex-char address', () => {
		expect(isValidEthAddress('0x097cD53Dc5Dda28c4f6A4431EA014916891beC02')).toBe(true);
	});

	it('accepts all-lowercase hex (EIP-55 casing not enforced for display)', () => {
		expect(isValidEthAddress('0x097cd53dc5dda28c4f6a4431ea014916891bec02')).toBe(true);
	});

	it('accepts all-uppercase hex', () => {
		expect(isValidEthAddress('0x097CD53DC5DDA28C4F6A4431EA014916891BEC02')).toBe(true);
	});

	it('rejects a missing 0x prefix', () => {
		expect(isValidEthAddress('097cD53Dc5Dda28c4f6A4431EA014916891beC02')).toBe(false);
	});

	it('rejects too-short hex (39 chars)', () => {
		expect(isValidEthAddress('0x097cD53Dc5Dda28c4f6A4431EA014916891beC0')).toBe(false);
	});

	it('rejects too-long hex (41 chars)', () => {
		expect(isValidEthAddress('0x097cD53Dc5Dda28c4f6A4431EA014916891beC020')).toBe(false);
	});

	it('rejects non-hex characters', () => {
		expect(isValidEthAddress('0xZZZcD53Dc5Dda28c4f6A4431EA014916891beC02')).toBe(false);
	});

	it('rejects whitespace / empty / null-ish inputs', () => {
		expect(isValidEthAddress('')).toBe(false);
		expect(isValidEthAddress('   ')).toBe(false);
		expect(isValidEthAddress('0x ')).toBe(false);
	});
});
