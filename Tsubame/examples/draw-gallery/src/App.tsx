import { createSignal } from 'solid-js';
import type { DrawPaintFunction } from '@torimi/tsubame-renderer-protocol';
import { GALLERY_PAINTERS, responsiveGrid } from './painters';

/**
 * draw ギャラリー（issue #732）。draw v1 語彙を横断するサンプル painter を、
 * `view` の `draw` property に載せて敷き詰める。同じ App が Hayate Renderer /
 * DOM Renderer の両経路で走り、同じ painter が同じ絵を出す（painter は
 * レンダラー非依存の純関数）。
 */

const BG = '#0b1020';
const CARD_BG = '#141a2e';
const INK = '#d8e2f2';
const MUTED = '#8ea1bb';
const ACCENT = '#14b8a6';

const CARD_W = 240;
const CARD_H = 150;

/** サイズ追従デモの選べる寸法（resize→paint ループの実地デモ）。 */
const RESIZE_STEPS = [
  { label: 'S', width: 160, height: 120 },
  { label: 'M', width: 300, height: 200 },
  { label: 'L', width: 460, height: 280 },
] as const;

function PainterCard(props: { title: string; blurb: string; paint: DrawPaintFunction }) {
  return (
    <view
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        padding: 12,
        borderRadius: 12,
        backgroundColor: CARD_BG,
      }}
    >
      <view style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
        <text style={{ color: INK, fontSize: 14, fontWeight: 700 }}>{props.title}</text>
        <text style={{ color: MUTED, fontSize: 11 }}>{props.blurb}</text>
      </view>
      {/* draw 面: painter はこの view のボーダーボックスサイズで呼ばれる。 */}
      <view
        draw={props.paint}
        style={{
          width: CARD_W,
          height: CARD_H,
          borderRadius: 8,
          overflow: 'hidden',
          backgroundColor: '#0a0f1e',
        }}
      />
    </view>
  );
}

export function DrawGalleryApp() {
  const [step, setStep] = createSignal(1);
  const current = () => RESIZE_STEPS[step()]!;

  return (
    <scroll-view
      style={{
        width: '100%',
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        gap: 24,
        padding: 24,
        backgroundColor: BG,
      }}
    >
      <view style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <text style={{ color: INK, fontSize: 24, fontWeight: 700 }}>Draw Gallery</text>
        <text style={{ color: MUTED, fontSize: 13 }}>
          {`${GALLERY_PAINTERS.length} sample painters — 同一 painter を選択中の renderer で描画`}
        </text>
      </view>

      {/* サンプル painter のカード群（wrap グリッド）。 */}
      <view style={{ display: 'flex', flexDirection: 'row', flexWrap: 'wrap', gap: 16 }}>
        {GALLERY_PAINTERS.map((entry) => (
          <PainterCard title={entry.title} blurb={entry.blurb} paint={entry.paint} />
        ))}
      </view>

      {/* サイズ追従デモ: ボックス寸法を変えると painter が別の絵を描き直す。 */}
      <view style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
        <text style={{ color: INK, fontSize: 16, fontWeight: 700 }}>
          Resize → paint（サイズ追従）
        </text>
        <view style={{ display: 'flex', flexDirection: 'row', gap: 8, alignItems: 'center' }}>
          {RESIZE_STEPS.map((s, i) => (
            <button
              onClick={() => setStep(i)}
              style={{
                paddingTop: 6,
                paddingBottom: 6,
                paddingLeft: 14,
                paddingRight: 14,
                borderRadius: 999,
                backgroundColor: step() === i ? ACCENT : '#1e2740',
                color: step() === i ? '#04110f' : INK,
                fontSize: 12,
                fontWeight: 700,
              }}
            >
              {s.label}
            </button>
          ))}
          <text style={{ color: MUTED, fontSize: 12 }}>
            {`${current().width} × ${current().height}`}
          </text>
        </view>
        <view
          draw={responsiveGrid}
          style={{
            width: current().width,
            height: current().height,
            borderRadius: 8,
            overflow: 'hidden',
            backgroundColor: '#0a0f1e',
          }}
        />
      </view>
    </scroll-view>
  );
}
