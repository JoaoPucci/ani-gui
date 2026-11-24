import { describe, expect, it, vi } from 'vitest';
import { revealFocused } from './strip-focus';

/** A card the row holds: whether it matches `:focus-visible`, and the
 *  scrolls asked of it. No layout engine runs here. */
function card(keyboard: boolean) {
	return {
		matches: (selector: string) => selector === ':focus-visible' && keyboard,
		scrollIntoView: vi.fn<(options: ScrollIntoViewOptions) => void>()
	};
}

describe('revealFocused', () => {
	it('brings a card the keyboard moved to just into view, past the row’s scroll padding', () => {
		const target = card(true);
		revealFocused(target, {}, 'smooth');
		expect(target.scrollIntoView).toHaveBeenCalledWith({
			block: 'nearest',
			inline: 'nearest',
			behavior: 'smooth'
		});
	});

	it('jumps rather than glides when told to (reduced motion)', () => {
		const target = card(true);
		revealFocused(target, {}, 'auto');
		expect(target.scrollIntoView).toHaveBeenCalledWith(
			expect.objectContaining({ behavior: 'auto' })
		);
	});

	it('leaves a card a click focused where it is', () => {
		const target = card(false);
		revealFocused(target, {}, 'auto');
		expect(target.scrollIntoView).not.toHaveBeenCalled();
	});

	it('leaves the row alone when the row itself takes focus', () => {
		const row = card(true);
		revealFocused(row, row, 'auto');
		expect(row.scrollIntoView).not.toHaveBeenCalled();
	});

	it('does nothing without a target', () => {
		expect(() => revealFocused(null, {}, 'auto')).not.toThrow();
	});
});
