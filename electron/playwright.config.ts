/**
 * Playwright e2e for the Electron app.
 *
 * The actual Electron launch happens inside each test, through the
 * shared helper `e2e/launch.ts` (which mutes audio) — this config
 * just sets defaults for test discovery, retries, and reports.
 *
 * Pre-test setup is the `pnpm run package:e2e` script run beforehand
 * (`pnpm e2e` chains it): the e2e package at
 * `dist-e2e/linux-unpacked/`, whose backend is the test-run build
 * `pnpm run build:backend:e2e` makes (lib/e2e-package.cjs).
 */
import { defineConfig } from '@playwright/test';

export default defineConfig({
	testDir: './e2e',
	timeout: 30_000,
	expect: { timeout: 5_000 },
	// Each test launches its own Electron process, so nothing here may
	// run concurrently. `fullyParallel: false` alone does NOT deliver
	// that — it only serializes tests WITHIN a file, while Playwright
	// still spreads separate spec files across `cpus/2` workers by
	// default (2 on a 4-core CI runner). That put smoke.spec.ts and
	// home-continue.spec.ts in flight together, each launching an app
	// under one Xvfb display, and intermittently killed one during
	// launch: `page.goto: Target page, context or browser has been
	// closed` plus a 30s worker-teardown timeout, with no assertion
	// failure. `workers: 1` is what actually serializes. Retries stay
	// at 0 so a real regression can't hide behind one.
	fullyParallel: false,
	workers: 1,
	retries: 0,
	reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
	use: {
		actionTimeout: 5_000,
		// Take a screenshot on failure for post-mortem debugging in CI.
		screenshot: 'only-on-failure',
		trace: 'retain-on-failure'
	}
});
