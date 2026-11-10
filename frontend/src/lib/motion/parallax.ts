/**
 * The hero image's scroll parallax, shared by Home and a show's page.
 *
 * With motion on the image drifts down a quarter of the distance
 * scrolled, up to 80px, behind the content. Under reduced motion it
 * holds its place. The scale is the image's bleed past the hero's
 * edges, not motion, and stays either way: a cover is barely
 * oversized, a fallback poster more so.
 */
export function heroParallaxTransform(scrollY: number, isCover: boolean, reduced: boolean): string {
	const offset = reduced ? 0 : Math.min(scrollY * 0.25, 80);
	const scale = isCover ? 1.02 : 1.15;
	return `translate3d(0, ${offset}px, 0) scale(${scale})`;
}
