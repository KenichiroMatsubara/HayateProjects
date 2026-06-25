// dev 変換（`jsx: "react-jsxdev"` / Vite dev）用に React の dev runtime を再 export する。
export { Fragment, jsxDEV } from 'react/jsx-dev-runtime';

// JSX 名前空間は production runtime と同一。重複を避けて再 export する。
export type { JSX } from './jsx-runtime.js';
