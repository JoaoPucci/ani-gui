/**
 * ETH-address helpers for the donation block on the About page.
 *
 * The address itself is hard-coded in the credits module; these
 * helpers exist to defend the surfaces around it:
 *   - `isValidEthAddress` is a sanity guard a future edit might
 *     trip if someone fat-fingers the constant. Strictly: 0x +
 *     exactly 40 hex chars, no case constraint (EIP-55 checksum
 *     casing is a separate concern we don't enforce on display).
 *   - `buildMetamaskSendUrl` produces the universal-link target
 *     used by the "Open in MetaMask" affordance. Mainnet chainId
 *     is appended via the @1 suffix per MetaMask's docs.
 */
import { describe, it, expect } from 'vitest';
import { isValidEthAddress, buildMetamaskSendUrl } from './eth';

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

describe('buildMetamaskSendUrl', () => {
	it('emits the universal-link target with @1 mainnet suffix', () => {
		// MetaMask's universal link format for a send intent. The @1
		// pins mainnet so wallets don't open with an arbitrary chain
		// preselected.
		expect(buildMetamaskSendUrl('0x097cD53Dc5Dda28c4f6A4431EA014916891beC02')).toBe(
			'https://metamask.app.link/send/0x097cD53Dc5Dda28c4f6A4431EA014916891beC02@1'
		);
	});

	it('refuses to build a URL for an invalid address — fails loud rather than emitting a broken link', () => {
		expect(() => buildMetamaskSendUrl('not-an-address')).toThrow();
	});
});
