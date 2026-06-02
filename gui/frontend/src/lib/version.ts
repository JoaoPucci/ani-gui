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

export const APP_VERSION: string = pkg.version;
