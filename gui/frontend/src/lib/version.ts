/**
 * Single source of truth for the app version string surfaced in UI.
 *
 * The version itself lives in `gui/frontend/package.json`. Reading it
 * once here (and re-exporting) lets deeply-nested routes consume it
 * without a `../../../package.json` chain — `arch/dep_direction`
 * caps relative imports at two `..` segments, so anything below
 * `src/routes/<dir>/` would otherwise have to inline its own import
 * helper.
 */
import pkg from '../../package.json';

/**
 * The raw semver — use this for any version *logic* (e.g. the
 * update-check comparison against GitHub release tags). Never decorate
 * it, or the comparison breaks.
 */
export const APP_VERSION: string = pkg.version;

/**
 * Append a `-dev` marker when running a dev build so it's visually
 * distinct from an installed release everywhere the version shows
 * (rail footer chip, About, Settings, Diagnostics). Display only —
 * keep `APP_VERSION` for version comparisons.
 */
export function versionLabel(version: string, isDev: boolean): string {
	return isDev ? `${version}-dev` : version;
}

/**
 * Display string for the running build. `import.meta.env.DEV` is true
 * under `vite dev` (the Electron dev launcher) and false in the
 * packaged `vite build` bundle, so released apps show the bare version.
 */
export const APP_VERSION_LABEL: string = versionLabel(APP_VERSION, import.meta.env.DEV);
