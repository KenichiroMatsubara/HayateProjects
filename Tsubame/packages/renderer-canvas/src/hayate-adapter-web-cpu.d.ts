declare module 'hayate-adapter-web-cpu' {
  export { HayateElementRenderer, HayateElementHtmlRenderer } from 'hayate-adapter-web';
  export default function init(): Promise<void>;
}
