#!/usr/bin/env node

import { generateCatalog } from './gen-catalog.mjs';
import { generateDelivery } from './gen-delivery.mjs';
import { generateWire } from './gen-wire.mjs';
import { writeIndex } from './gen-index.mjs';

generateWire();
generateCatalog();
generateDelivery();
writeIndex();
console.log('Generated Tsubame/proto/generated/*');
