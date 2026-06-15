// The Style Channel gate is the shared, spec-generated rule (Tsubame ADR-0008);
// both renderers import the same function so DOM and Canvas cannot diverge.
export { shouldApplyTextLocalPatch } from '@tsubame/renderer-protocol';
