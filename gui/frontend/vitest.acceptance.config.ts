import { paraglideVitePlugin } from '@inlang/paraglide-js';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

/**
 * The route-level acceptance tier `docs/testing.md` describes: real
 * route components mounted into a DOM, composed over the real
 * `$lib` modules, with only the HTTP boundary faked by MSW.
 *
 * It gets its own config rather than a project inside `vite.config.ts`
 * because the two tiers disagree about the environment (`node` vs a
 * DOM) and about module resolution, and because the unit config also
 * carries the coverage settings the ratchet reads — this tier must not
 * be able to move those numbers.
 */
export default defineConfig({
	plugins: [
		paraglideVitePlugin({
			project: './project.inlang',
			outdir: './src/lib/paraglide',
			strategy: ['localStorage', 'preferredLanguage', 'baseLocale']
		}),
		sveltekit()
	],
	// Without this, `svelte`'s package exports resolve to the SSR
	// build and `mount()` throws `lifecycle_function_unavailable` —
	// the server build has no lifecycle at all. Vitest is a Node
	// process, so the browser condition has to be asked for.
	resolve: { conditions: ['browser'] },
	test: {
		// happy-dom, not jsdom: it is the DOM implementation already
		// in devDependencies, so this tier costs no new environment.
		environment: 'happy-dom',
		include: ['tests/acceptance/**/*.{test,spec}.ts'],
		setupFiles: ['tests/acceptance/setup.ts'],
		// Scenarios wait on real deadlines inside the app — the search
		// filter's grace render is 2s on its own. Above vitest's 5s
		// default so a scenario's own wait expires first and reports
		// what it was waiting for, instead of a bare "test timed out".
		testTimeout: 20_000
	}
});
