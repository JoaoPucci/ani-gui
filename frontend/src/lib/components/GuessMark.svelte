<!--
  GuessMark — the app's one mark for "guessed": two waves, ≈, drawn in
  the stroke the player's ⋯ menu icons use (24-unit box, 2.25 stroke,
  round caps). Every surface that says a show was guessed carries it:
  the player's slip and its ⋯ entry, the Continue card's pill and the
  failed resume's button. Decorative: the words beside it say what it
  means.
-->
<script lang="ts">
	let { size = 18, class: className = '' }: { size?: number; class?: string } = $props();
</script>

<svg
	class="guess-mark {className}"
	viewBox="0 0 24 24"
	width={size}
	height={size}
	aria-hidden="true"
	data-guess-mark
>
	<path
		d="M4.5 9.25c2.5-2.33 5-2.33 7.5 0s5 2.33 7.5 0M4.5 15.25c2.5-2.33 5-2.33 7.5 0s5 2.33 7.5 0"
		fill="none"
		stroke="currentColor"
		stroke-width="2.25"
		stroke-linecap="round"
		stroke-linejoin="round"
	/>
</svg>

<style>
	.guess-mark {
		flex-shrink: 0;
	}
</style>
