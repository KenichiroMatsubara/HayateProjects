#!/usr/bin/env node

import { generateCatalog } from './gen-catalog.mjs';
import { generateCodec } from './gen-codec.mjs';
import { generateDelivery } from './gen-delivery.mjs';
import { generateWire } from './gen-wire.mjs';
import { writeIndex } from './gen-index.mjs';

generateWire();
generateCatalog();
generateCodec();
generateDelivery();
writeIndex();
console.log('Generated Tsubame/proto/generated/*');
