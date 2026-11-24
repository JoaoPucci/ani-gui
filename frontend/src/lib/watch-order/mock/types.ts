/**
 * The watch-order mockups' shapes: the real feature's entry and run,
 * with the fixtures' made-up progress carried on the run (the real
 * feature derives progress from the watch history, apart from the run).
 */
import type { FixtureRun } from '../fixtures/runs';
import type { EntryProgress, WatchEntry } from '../types';

export type MockEntry = WatchEntry;
export type MockProgress = EntryProgress;
export type MockRun = FixtureRun;
