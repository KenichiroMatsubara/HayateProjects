import { useReducer, useState } from 'react';
import type {
  HayateCssStyle,
  InteractionEvent,
} from '@torimi/tsubame-renderer-protocol';
import { SketchDocument, type Sample } from './sketch-document.js';

const C = {
  paper: '#f7f3ea',
  panel: '#fffdf8ee',
  ink: '#171a21',
  muted: '#6c706f',
  line: '#d9d3c8',
  accent: '#f06449',
  accentSoft: '#fbe1d9',
  shadow: '#2b211a24',
} as const;

const THIN_WIDTH = 5;
const THICK_WIDTH = 11;

function sampleOf(event: InteractionEvent): Sample | null {
  return event.x === undefined || event.y === undefined
    ? null
    : { x: event.x, y: event.y };
}

export function App() {
  const [document] = useState(() => new SketchDocument());
  const [, redraw] = useReducer((revision: number) => revision + 1, 0);

  const updateFromPointer = (
    event: InteractionEvent,
    update: (sample: Sample) => boolean,
  ) => {
    const sample = sampleOf(event);
    if (sample !== null && update(sample)) redraw();
  };

  const undo = () => {
    if (document.undo()) redraw();
  };

  const clear = () => {
    if (document.clear()) redraw();
  };

  const toggleWidth = () => {
    const next = document.strokeWidth === THIN_WIDTH ? THICK_WIDTH : THIN_WIDTH;
    if (document.setStrokeWidth(next)) redraw();
  };

  return (
    <view style={shell} draw={document.frame()}>
      <view
        style={drawingSurface}
        user-select="none"
        onPointerDown={(event) => updateFromPointer(event, (sample) => document.begin(sample))}
        onPointerMove={(event) => updateFromPointer(event, (sample) => document.append(sample))}
        onPointerUp={(event) => updateFromPointer(event, (sample) => document.end(sample))}
      />

      <view style={topBar}>
        <view style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
          <text style={title}>Sketch</text>
          <text style={status}>{`${document.strokeCount} strokes`}</text>
        </view>
        <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <button style={toolButton} onClick={toggleWidth}>
            {document.strokeWidth === THIN_WIDTH ? '細' : '太'}
          </button>
          <button style={toolButton} onClick={undo}>Undo</button>
          <button style={clearButton} onClick={clear}>Clear</button>
        </view>
      </view>

      <view style={hintPill}>
        <text style={hint}>1本指で描く</text>
      </view>
    </view>
  );
}

const shell: HayateCssStyle = {
  width: '100%',
  height: '100%',
  display: 'flex',
  backgroundColor: C.paper,
  defaultColor: C.ink,
  defaultFontFamily: 'Inter, Segoe UI, system-ui, sans-serif',
  overflow: 'hidden',
};

const drawingSurface: HayateCssStyle = {
  position: 'absolute',
  top: 0,
  left: 0,
  width: '100%',
  height: '100%',
  backgroundColor: 'transparent',
  cursor: 'crosshair',
  zIndex: 0,
};

const topBar: HayateCssStyle = {
  position: 'absolute',
  top: 12,
  left: 12,
  right: 12,
  height: 64,
  display: 'flex',
  flexDirection: 'row',
  alignItems: 'center',
  justifyContent: 'space-between',
  paddingLeft: 16,
  paddingRight: 10,
  backgroundColor: C.panel,
  borderRadius: 18,
  borderWidth: 1,
  borderStyle: 'solid',
  borderColor: C.line,
  boxShadow: [{ offsetX: 0, offsetY: 8, blur: 24, spread: -8, color: C.shadow, inset: false }],
  zIndex: 10,
};

const title: HayateCssStyle = {
  defaultColor: C.ink,
  defaultFontSize: 19,
  fontWeight: 750,
};

const status: HayateCssStyle = {
  defaultColor: C.muted,
  defaultFontSize: 11,
};

const toolButton: HayateCssStyle = {
  height: 40,
  paddingLeft: 13,
  paddingRight: 13,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  backgroundColor: '#ffffff',
  defaultColor: C.ink,
  defaultFontSize: 12,
  fontWeight: 650,
  borderRadius: 12,
  borderWidth: 1,
  borderStyle: 'solid',
  borderColor: C.line,
  cursor: 'pointer',
  ':active': { backgroundColor: C.accentSoft },
};

const clearButton: HayateCssStyle = {
  ...toolButton,
  backgroundColor: C.accent,
  borderColor: C.accent,
  defaultColor: '#ffffff',
};

const hintPill: HayateCssStyle = {
  position: 'absolute',
  bottom: 18,
  left: 0,
  right: 0,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  zIndex: 10,
};

const hint: HayateCssStyle = {
  paddingTop: 8,
  paddingBottom: 8,
  paddingLeft: 14,
  paddingRight: 14,
  backgroundColor: '#fffdf8cc',
  defaultColor: C.muted,
  defaultFontSize: 11,
  borderRadius: 999,
};
