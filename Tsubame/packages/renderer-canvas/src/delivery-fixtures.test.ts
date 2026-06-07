import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import type { EventPayload } from '@tsubame/protocol-generated/protocol';
import { parseEvent } from '@tsubame/protocol-generated/protocol';

const fixturesPath = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../../../Hayate/proto/spec/fixtures/delivery_encode.json',
);

interface DeliveryFixture {
  readonly name: string;
  readonly kind: EventPayload['kind'];
  readonly fields: Record<string, string | number>;
  readonly wire: readonly (string | number)[];
}

const fixtures = JSON.parse(readFileSync(fixturesPath, 'utf8')) as DeliveryFixture[];

function eventFields(ev: EventPayload): Record<string, string | number> {
  switch (ev.kind) {
    case 'click':
      return { target_id: ev.targetId, x: ev.x, y: ev.y };
    case 'focus':
    case 'blur':
    case 'active_end':
    case 'hover_enter':
    case 'hover_leave':
    case 'active_start':
      return { target_id: ev.targetId };
    case 'text_input':
    case 'composition_start':
    case 'composition_update':
    case 'composition_end':
      return { target_id: ev.targetId, text: ev.text };
    case 'scroll':
      return { target_id: ev.targetId, delta_x: ev.deltaX, delta_y: ev.deltaY };
    case 'resize':
      return { width: ev.width, height: ev.height };
    case 'key_down':
      return { target_id: ev.targetId, key: ev.key, modifiers: ev.modifiers };
    case 'pointer_move':
      return { x: ev.x, y: ev.y };
    case 'fetch_font':
      return { family: ev.family };
    default: {
      const _exhaustive: never = ev;
      return _exhaustive;
    }
  }
}

describe('delivery fixtures (C5)', () => {
  for (const fixture of fixtures) {
    it(fixture.name, () => {
      const event = parseEvent([...fixture.wire]);
      expect(event.kind).toBe(fixture.kind);
      expect(event.value).toBe(fixture.wire[0]);
      expect(eventFields(event)).toEqual(fixture.fields);
    });
  }
});
