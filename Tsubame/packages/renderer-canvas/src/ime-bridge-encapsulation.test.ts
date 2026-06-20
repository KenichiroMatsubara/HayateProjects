// Enforcement guard (ADR-0069, #392): the platform `EditContext` — creating it
// (`new EditContext`) and attaching/detaching it (`.editContext`) — may be
// touched only by the web IME bridge module (`edit-context-sync.ts`). Attaching
// an EditContext is what raises the mobile soft keyboard, so confining it to one
// module keyed off `raw.ime_wants_keyboard()` is what stops a plain tap from
// summoning the keyboard (#392) from regressing. Production code elsewhere must
// route through `attachTextInput` / `syncEditContext` instead.
//
// Test files are exempt: they legitimately stub a host-managed EditContext to
// exercise the candidate-window path.

import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, it, expect } from 'vitest';

const SRC = dirname(fileURLToPath(import.meta.url));

/** The one module allowed to touch the platform EditContext. */
const BRIDGE_FILE = 'edit-context-sync.ts';

/** Substrings that mean "directly touching the platform EditContext API".
 * Chosen so identifiers like `syncEditContext` / `editContexts` don't trip it. */
const FORBIDDEN = ['new EditContext', '.editContext'];

function tsFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...tsFiles(path));
    } else if (entry.name.endsWith('.ts') && !entry.name.endsWith('.test.ts')) {
      out.push(path);
    }
  }
  return out;
}

describe('EditContext encapsulation (#392)', () => {
  it('confines direct EditContext access to the bridge module', () => {
    const violations: string[] = [];
    for (const file of tsFiles(SRC)) {
      if (file.endsWith(BRIDGE_FILE)) continue;
      const lines = readFileSync(file, 'utf8').split('\n');
      lines.forEach((line, i) => {
        const trimmed = line.trimStart();
        if (trimmed.startsWith('//') || trimmed.startsWith('*')) return;
        for (const needle of FORBIDDEN) {
          if (line.includes(needle)) {
            violations.push(`${file.slice(SRC.length + 1)}:${i + 1}: ${line.trim()}`);
          }
        }
      });
    }

    expect(
      violations,
      `direct EditContext access must live only in ${BRIDGE_FILE} ` +
        `(route through attachTextInput / syncEditContext):\n${violations.join('\n')}`,
    ).toEqual([]);
  });
});
