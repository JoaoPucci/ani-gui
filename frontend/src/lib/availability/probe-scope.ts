/**
 * The page an availability probe was asked for.
 */

export interface ProbeScope {
	signal(): AbortSignal;
	settle(): void;
}

export function createProbeScope(currentPath: () => string): ProbeScope {
	void currentPath;
	const never = new AbortController();
	return {
		signal: () => never.signal,
		settle: () => {}
	};
}

export const probeScope = createProbeScope(() =>
	typeof location === 'undefined' ? '' : location.pathname
);
