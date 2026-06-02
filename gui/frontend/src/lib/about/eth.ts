/**
 * ETH-address helpers for the About page donation block.
 *
 * Strictly minimal: a regex-based shape guard. EIP-55 checksum casing
 * is not enforced — addresses on display can be either canonical or
 * mixed-case, and we don't want to reject a hand-pasted address for
 * the wrong reason.
 */

const ADDRESS_RE = /^0x[0-9a-fA-F]{40}$/;

/** True iff `value` is a 0x-prefixed 40-hex-char Ethereum address.
 *  Mixed casing is accepted; whitespace is not. */
export function isValidEthAddress(value: string): boolean {
	return typeof value === 'string' && ADDRESS_RE.test(value);
}
