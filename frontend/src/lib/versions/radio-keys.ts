/**
 * Arrow-key movement in a radio group, as WAI-ARIA describes it: the
 * arrows move the selection to the next or previous radio, wrapping
 * at either end, and Home and End jump to the ends. The horizontal
 * arrows follow the reading direction, so in a right-to-left layout
 * they swap.
 *
 * The index the key moves the selection to, or null for a key the
 * group does not answer.
 */
export function radioKeyTarget(
	key: string,
	index: number,
	count: number,
	rtl = false
): number | null {
	if (count < 1) return null;
	const step = (by: number) => (index + by + count) % count;
	const inline = rtl ? -1 : 1;
	switch (key) {
		case 'ArrowDown':
			return step(1);
		case 'ArrowUp':
			return step(-1);
		case 'ArrowRight':
			return step(inline);
		case 'ArrowLeft':
			return step(-inline);
		case 'Home':
			return 0;
		case 'End':
			return count - 1;
		default:
			return null;
	}
}

/** The parts of a keydown a radio's handler reads. */
export interface RadioKeyEvent {
	key: string;
	currentTarget: EventTarget | null;
	preventDefault(): void;
}

const readsRightToLeft = (el: Element) => getComputedStyle(el).direction === 'rtl';

/**
 * A radio's keydown: move the choice to the radio the key names,
 * `choose` it, and put focus on it — its siblings are the group's
 * radios, in order. Any other key is left alone.
 */
export function handleRadioKey(
	e: RadioKeyEvent,
	index: number,
	count: number,
	choose: (index: number) => void,
	isRtl: (el: Element) => boolean = readsRightToLeft
): void {
	const radio = e.currentTarget as HTMLElement | null;
	const next = radioKeyTarget(e.key, index, count, radio ? isRtl(radio) : false);
	if (next === null) return;
	e.preventDefault();
	choose(next);
	(radio?.parentElement?.children[next] as HTMLElement | undefined)?.focus();
}
