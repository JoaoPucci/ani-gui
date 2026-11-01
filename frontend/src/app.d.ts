// See https://svelte.dev/docs/kit/types#app for information about these
// interfaces. Empty stubs keep TypeScript happy until we need them.
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		/** What the layout keeps on each history entry through
		 *  SvelteKit's shallow-routing state ($lib/history/nav-depth). */
		interface PageState {
			aniGuiDepth?: number;
			aniGuiFrom?: string;
		}
		// interface Platform {}
	}
}

export {};
