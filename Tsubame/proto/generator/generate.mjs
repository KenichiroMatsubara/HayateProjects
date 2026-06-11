#!/usr/bin/env node

import { generateCatalog } from './gen-catalog.mjs';
import { generateCodec } from './gen-codec.mjs';
import { generateDelivery } from './gen-delivery.mjs';
import { generateWire } from './gen-wire.mjs';
import { writeIndex } from './gen-index.mjs';
import { generateStyleTypes } from './gen-style-types.mjs';
import { generateEventKind } from './gen-event-kind.mjs';
import { generatePseudoState } from './gen-pseudo-state.mjs';

generateWire();
generateCatalog();
generateCodec();
generateDelivery();
writeIndex();
generateStyleTypes();
generateEventKind();
generatePseudoState();
console.log('Generated Tsubame/proto/generated/*');
console.log('Generated Tsubame/packages/renderer-protocol/src/generated/*');
