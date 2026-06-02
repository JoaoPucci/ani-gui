/**
 * ETH-address helpers for the About page donation block.
 *
 * Strictly minimal: a regex-based shape guard and a builder for
 * MetaMask's universal-link "Send" target. EIP-55 checksum casing
 * is not enforced — addresses on display can be either canonical
 * or mixed-case, and we don't want to reject a hand-pasted address
 * for the wrong reason.
 */

const ADDRESS_RE = /^0x[0-9a-fA-F]{40}$/;

/** True iff `value` is a 0x-prefixed 40-hex-char Ethereum address.
 *  Mixed casing is accepted; whitespace is not. */
export function isValidEthAddress(value: string): boolean {
	return typeof value === 'string' && ADDRESS_RE.test(value);
}

/** Build the MetaMask universal-link "Send" URL for `address`. The
 *  `@1` chain suffix pins mainnet so the wallet opens with the
 *  intended chain rather than whatever the user last switched to.
 *
 *  Throws `Error` on an invalid address — preferable to silently
 *  emitting a dead link the donate button would route to. */
export function buildMetamaskSendUrl(address: string): string {
	if (!isValidEthAddress(address)) {
		throw new Error(`buildMetamaskSendUrl: invalid Ethereum address "${address}"`);
	}
	return `https://metamask.app.link/send/${address}@1`;
}
