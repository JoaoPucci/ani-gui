/**
 * The seed fast-check generates property inputs from, pinned for the
 * coverage run.
 *
 * Property tests reach some lines only when a run happens to generate
 * the input that does, and a file's coverage decides its CRAP score,
 * so with a fresh seed each run a file near the high-risk bar crossed
 * it or not by chance. `vite.config.ts` hands the coverage run
 * `FAST_CHECK_SEED`, and this setup file gives it to fast-check before
 * any test file runs; the ordinary test runs set no seed and keep a
 * fresh one each time, which is where the properties earn their
 * variety.
 */
import fc from 'fast-check';

/** The seed `env` names, when it names one that is a whole number. */
export function pinnedSeed(env: Record<string, string | undefined>): number | undefined {
	const raw = env.FAST_CHECK_SEED;
	if (raw === undefined || !/^-?\d+$/.test(raw)) return undefined;
	return Number(raw);
}

const seed = pinnedSeed(process.env);
if (seed !== undefined) fc.configureGlobal({ seed });
