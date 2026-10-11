// The packaged window loads a static build with no SSR. Prerendering
// is off for every route, so adapter-static emits the one index.html
// fallback the window loads, and the SvelteKit router handles every
// route on the client.
export const prerender = false;
export const ssr = false;
export const csr = true;
