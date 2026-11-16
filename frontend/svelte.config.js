import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		// SPA mode: every route falls back to index.html and SvelteKit's
		// client-side router takes over. The packaged Electron window
		// loads index.html via `file://`, where absolute paths like
		// `/_app/foo.js` resolve to filesystem-root nonsense — so we
		// emit relative paths (`./_app/foo.js`). Vite dev still resolves
		// them against `localhost:5173`.
		adapter: adapter({
			fallback: 'index.html',
			pages: 'build',
			assets: 'build',
			strict: false
		}),
		paths: {
			relative: true
		},
		// The renderer's content security policy, the half the build owns:
		// SvelteKit writes it into every page as a <meta http-equiv>, with
		// the hash of its own inline bootstrap script added to script-src
		// (the hash changes with every build). The main process sends the
		// other half as a header — the fetch directives, which name the
		// backend's loopback lanes and can only be known at launch
		// (electron/lib/renderer-csp.cjs). A resource has to pass both.
		csp: {
			mode: 'hash',
			directives: {
				'script-src': ['self'],
				'object-src': ['none'],
				'base-uri': ['none']
			}
		},
		alias: {
			$lib: 'src/lib'
		}
	}
};

export default config;
