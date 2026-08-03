import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import {
  BUNDLE_ROUTE,
  HAYATE_HOST_GLOBAL,
  LOG_LEVEL_ERROR,
  isKnownLogLevel,
  validateDemoManifest,
  validateLogBatch,
  validateLogEntry,
} from './generated.js';

describe('generated Torimi wire projection', () => {
  it('projects canonical route, global, open-vocabulary, and validator behavior', () => {
    expect(BUNDLE_ROUTE).toBe('/bundle.js');
    expect(HAYATE_HOST_GLOBAL).toBe('__hayateHost');
    expect(LOG_LEVEL_ERROR).toBe('error');
    expect(isKnownLogLevel('error')).toBe(true);
    expect(isKnownLogLevel('trace')).toBe(false);

    expect(
      validateLogEntry({
        seq: 1,
        ts: 1720000000000,
        source: 'future-native-source',
        level: 'trace',
        message: 'forward-compatible',
        futureField: true,
      }),
    ).toEqual({
      ok: true,
      value: {
        seq: 1,
        ts: 1720000000000,
        source: 'future-native-source',
        level: 'trace',
        message: 'forward-compatible',
        futureField: true,
      },
    });
  });
});

describe('human-reviewed fixture parity', () => {
  it('matches every shared accept/reject expectation without copying input values into issues', async () => {
    const corpus = JSON.parse(
      await readFile(new URL('../fixtures/parity.json', import.meta.url), 'utf8'),
    ) as Record<string, Array<{ name: string; valid: boolean; path?: string; value: unknown }>>;
    const validators = {
      logEntry: validateLogEntry,
      logBatch: validateLogBatch,
      demoManifest: validateDemoManifest,
    } as const;

    for (const [dto, validate] of Object.entries(validators)) {
      for (const fixture of corpus[dto] ?? []) {
        const result = validate(fixture.value);
        expect(result.ok, `${dto}: ${fixture.name}`).toBe(fixture.valid);
        if (!result.ok && fixture.path !== undefined) {
          expect(result.issues.map((issue) => issue.path), `${dto}: ${fixture.name}`).toContain(
            fixture.path,
          );
          expect(result.issues.every((issue) => !('value' in issue))).toBe(true);
        }
      }
    }
  });
});
