declare module 'hayate-adapter-web-null' {
  export { HayateElementRenderer } from 'hayate-adapter-web';
  export function initSync(module: { module: BufferSource }): void;
  export default function init(): Promise<void>;
}
