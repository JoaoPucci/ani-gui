// WCAG 2.x contrast arithmetic for the design-token tests.
//
// The ratio is the one WCAG 2.2 defines (success criterion 1.4.3):
// relative luminance from linearised sRGB channels, then
// (lighter + 0.05) / (darker + 0.05), so it runs from 1 (identical) to
// 21 (black on white).
//
// Translucent text is resolved before it is measured: a colour drawn
// at alpha `a` over a surface is the surface's channels moved `a` of
// the way to the colour's, channel by channel in gamma-encoded sRGB.
// That is how the browser composites a translucent colour onto what is
// under it, so the ratio measured is the one a reader sees.

/** An sRGB colour, each channel 0–255. */
export type Rgb = readonly [number, number, number];

/** WCAG AA minimum for normal-size text. */
export const AA_TEXT = 4.5;
/** WCAG AA minimum for large text: 24px and up, or 18.66px (14pt) and up when bold. */
export const AA_LARGE_TEXT = 3;

/** Parses `#rgb` or `#rrggbb`; anything else is an error, never a guess. */
export function parseHex(value: string): Rgb {
	const short = /^#([0-9a-f])([0-9a-f])([0-9a-f])$/i.exec(value);
	if (short) return [0, 1, 2].map((i) => parseInt(short[i + 1].repeat(2), 16)) as unknown as Rgb;
	const long = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(value);
	if (long) return [0, 1, 2].map((i) => parseInt(long[i + 1], 16)) as unknown as Rgb;
	throw new Error(`not a hex colour: ${value}`);
}

function linear(channel: number): number {
	const c = channel / 255;
	return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/** WCAG relative luminance, 0 (black) to 1 (white). */
export function relativeLuminance([r, g, b]: Rgb): number {
	return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
}

/** The colour seen where `fg` at `alpha` (0–1) is drawn over an opaque `bg`. */
export function composite(fg: Rgb, alpha: number, bg: Rgb): Rgb {
	if (!(alpha >= 0 && alpha <= 1)) throw new Error(`alpha out of range: ${alpha}`);
	return [0, 1, 2].map((i) => fg[i] * alpha + bg[i] * (1 - alpha)) as unknown as Rgb;
}

/** WCAG contrast ratio between two opaque colours, 1 to 21. Order does not matter. */
export function contrastRatio(a: Rgb, b: Rgb): number {
	const la = relativeLuminance(a);
	const lb = relativeLuminance(b);
	return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/** Contrast of `text` drawn at `alpha` over the opaque `surface`. */
export function textContrast(text: Rgb, alpha: number, surface: Rgb): number {
	return contrastRatio(composite(text, alpha, surface), surface);
}
