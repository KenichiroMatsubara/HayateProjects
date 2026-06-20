import type { HayateCssStyle, ViewportCondition } from '@tsubame/renderer-protocol';
import { DEFAULT_ACCENT, DEFAULT_THEME, inputStyle, palette, type Palette } from '../theme';

// ─────────────────────────────────────────────────────────────────────────────
// Data-driven CSS Gallery (issue #246)
//
// Each Hayate CSS property gets a POP card whose body renders the property
// *actually in effect*. The gallery is described as data so the live coverage
// is inspectable: `GALLERY_LIVE_PROPERTIES` is derived from the same card
// descriptors that render on screen, and css-gallery.test.ts asserts it covers
// every property in the protocol catalog. Card titles use the catalog patchKey
// so the chip you read matches the key you'd write.
//
// Colours come from the resolved theme palette (issue #249): the sections are
// built per-render from the active palette so theme/accent switches re-apply.
// ─────────────────────────────────────────────────────────────────────────────

export interface GalleryCard {
  /** Header chip text. Usually the catalog patchKey it demonstrates. */
  title: string;
  /** Catalog patchKeys this card shows live. Empty for renderer-feature cards. */
  properties: readonly string[];
  /** Optional caption under the live body. */
  note?: string;
  render: () => unknown;
}

export interface GallerySection {
  title: string;
  /** Playful per-section accent used on the rail, chip dot, and title. */
  accent: string;
  cards: readonly GalleryCard[];
}

/**
 * `@media` ブレークポイントのライブ実証（ADR-0081）。Hayate CSS には
 * スタイルシートが無いため、media は raw CSS ではなく `styleVariants` という
 * 型付き宣言で要素ごとに載せる。DOM Renderer ではこれが本物の
 * `@media (min-width: …)` ルールにコンパイルされ（DevTools の
 * `<style data-tsubame-variant>` で確認できる）、Canvas Renderer では viewport で
 * 評価される。ウィンドウ幅を変えると、現在マッチする帯のタイルだけが点灯する。
 *
 * 帯は元デモ（gomi/todo-demo-v2.css の `.mq-tile`）と同じ S(<720) / M(720–1099) /
 * L(≥1100) の 3 段。各タイルは base が `muted`、自帯の variant でだけ `accent` に
 * なる。`defaultColor` は ambient チャネルなので子 `text` まで継承する。
 */
const MQ_TILES: readonly { label: string; condition: ViewportCondition }[] = [
  { label: 'S  < 720', condition: { maxWidth: 719 } },
  { label: 'M  720–1099', condition: { minWidth: 720, maxWidth: 1099 } },
  { label: 'L  ≥ 1100', condition: { minWidth: 1100 } },
];

export function MediaTiles(props: { colors: Palette }) {
  const p = props.colors;
  return (
    <view style={{ display: 'flex', flexDirection: 'column', gap: 6, width: 200 }}>
      {MQ_TILES.map((tile) => (
        <view
          style={{
            height: 34,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: p.panel2,
            defaultColor: p.muted,
            defaultFontSize: 12,
            borderRadius: 8,
            borderWidth: 1,
            borderStyle: 'solid',
            borderColor: p.line,
          }}
          styleVariants={[
            {
              condition: tile.condition,
              style: { backgroundColor: p.accent, defaultColor: p.black, borderColor: p.accent },
            },
          ]}
        >
          <text>{tile.label}</text>
        </view>
      ))}
    </view>
  );
}

export function SampleBox(props: { colors: Palette; label: string; style: HayateCssStyle }) {
  return (
    <view style={{
      width: 120,
      height: 56,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: props.colors.panel2,
      borderWidth: 1,
      borderColor: props.colors.line,
      borderRadius: 10,
      ...props.style,
    }}>
      <text style={{ color: props.colors.text, fontSize: 12 }}>{props.label}</text>
    </view>
  );
}

export function buildSections(p: Palette): readonly GallerySection[] {
  return [
    {
      title: 'Visual',
      accent: p.accent,
      cards: [
        {
          title: 'backgroundColor',
          properties: ['backgroundColor'],
          render: () => <SampleBox colors={p} label="Sample" style={{ backgroundColor: p.accent }} />,
        },
        {
          title: 'opacity',
          properties: ['opacity'],
          render: () => <SampleBox colors={p} label="0.45" style={{ opacity: 0.45 }} />,
        },
        {
          title: 'borderRadius',
          properties: ['borderRadius'],
          render: () => <SampleBox colors={p} label="r16" style={{ borderRadius: 16 }} />,
        },
        {
          title: 'borderWidth',
          properties: ['borderWidth'],
          render: () => <SampleBox colors={p} label="3px" style={{ borderWidth: 3, borderColor: p.accent }} />,
        },
        {
          title: 'borderColor',
          properties: ['borderColor'],
          render: () => <SampleBox colors={p} label="violet" style={{ borderWidth: 2, borderColor: p.violet }} />,
        },
        {
          title: 'borderStyle',
          properties: ['borderStyle'],
          note: 'solid / dashed',
          render: () => (
            <view style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              <SampleBox colors={p} label="solid" style={{ borderWidth: 2, borderStyle: 'solid', borderColor: p.accent }} />
              <SampleBox colors={p} label="dashed" style={{ borderWidth: 2, borderStyle: 'dashed', borderColor: p.accent2 }} />
            </view>
          ),
        },
        {
          title: 'boxShadow',
          properties: ['boxShadow'],
          note: 'elevation + inset ring — ADR-0095',
          render: () => (
            <view style={{ display: 'flex', flexDirection: 'column', gap: 10, padding: 6 }}>
              <SampleBox
                colors={p}
                label="lift"
                style={{ boxShadow: [{ offsetX: 0, offsetY: 6, blur: 16, spread: 0, color: p.shadow, inset: false }] }}
              />
              <SampleBox
                colors={p}
                label="inset"
                style={{ boxShadow: [{ offsetX: 0, offsetY: 0, blur: 0, spread: 3, color: p.accent, inset: true }] }}
              />
            </view>
          ),
        },
      ],
    },
    {
      title: 'Sizing',
      accent: p.blue,
      cards: ([
        ['width', { width: 140 }],
        ['height', { height: 72 }],
        ['minWidth', { minWidth: 120, width: 80 }],
        ['minHeight', { minHeight: 64, height: 40 }],
        ['maxWidth', { maxWidth: 90, width: 140 }],
        ['maxHeight', { maxHeight: 40, height: 72 }],
      ] as const).map(([name, style]) => ({
        title: name,
        properties: [name],
        render: () => <SampleBox colors={p} label="Sample" style={style} />,
      })),
    },
    {
      title: 'Spacing',
      accent: p.violet,
      cards: [
        ...(['padding', 'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft'] as const).map((key) => ({
          title: key,
          properties: [key] as readonly string[],
          render: () => (
            <view style={{
              backgroundColor: p.panel2,
              borderWidth: 1,
              borderColor: p.line,
              borderRadius: 8,
              [key]: 14,
            }}>
              <view style={{ backgroundColor: p.accent, height: 28, width: 80, borderRadius: 6 }} />
            </view>
          ),
        })),
        ...(['margin', 'marginTop', 'marginRight', 'marginBottom', 'marginLeft'] as const).map((key) => ({
          title: key,
          properties: [key] as readonly string[],
          render: () => (
            <view style={{ backgroundColor: p.black, padding: 4, borderRadius: 8 }}>
              <view style={{
                backgroundColor: p.panel2,
                borderWidth: 1,
                borderColor: p.line,
                borderRadius: 6,
                height: 28,
                width: 80,
                [key]: 10,
              }} />
            </view>
          ),
        })),
        {
          title: 'gap',
          properties: ['gap'],
          render: () => (
            <view style={{
              display: 'flex',
              flexDirection: 'row',
              gap: 16,
              backgroundColor: p.panel2,
              padding: 8,
              borderRadius: 8,
              borderWidth: 1,
              borderColor: p.line,
            }}>
              <view style={{ width: 36, height: 24, backgroundColor: p.accent, borderRadius: 6 }} />
              <view style={{ width: 36, height: 24, backgroundColor: p.blue, borderRadius: 6 }} />
            </view>
          ),
        },
      ],
    },
    {
      title: 'Flex & Grid',
      accent: p.accent2,
      cards: [
        {
          title: 'display',
          properties: ['display'],
          render: () => (
            <view style={{ display: 'flex', flexDirection: 'row', gap: 6 }}>
              <view style={{ width: 24, height: 24, backgroundColor: p.accent, borderRadius: 6 }} />
              <view style={{ width: 24, height: 24, backgroundColor: p.blue, borderRadius: 6 }} />
            </view>
          ),
        },
        {
          title: 'flexDirection',
          properties: ['flexDirection'],
          render: () => (
            <view style={{ display: 'flex', flexDirection: 'column', gap: 6, height: 72 }}>
              <view style={{ width: 48, height: 16, backgroundColor: p.accent, borderRadius: 4 }} />
              <view style={{ width: 48, height: 16, backgroundColor: p.blue, borderRadius: 4 }} />
            </view>
          ),
        },
        {
          title: 'flexWrap',
          properties: ['flexWrap'],
          render: () => (
            <view style={{ display: 'flex', flexWrap: 'wrap', width: 120, gap: 4 }}>
              <view style={{ width: 48, height: 20, backgroundColor: p.accent, borderRadius: 4 }} />
              <view style={{ width: 48, height: 20, backgroundColor: p.blue, borderRadius: 4 }} />
              <view style={{ width: 48, height: 20, backgroundColor: p.violet, borderRadius: 4 }} />
            </view>
          ),
        },
        {
          title: 'alignItems',
          properties: ['alignItems'],
          render: () => (
            <view style={{
              display: 'flex',
              flexDirection: 'row',
              alignItems: 'center',
              gap: 6,
              height: 56,
              backgroundColor: p.panel2,
              borderRadius: 8,
            }}>
              <view style={{ width: 20, height: 20, backgroundColor: p.accent, borderRadius: 4 }} />
              <view style={{ width: 20, height: 36, backgroundColor: p.blue, borderRadius: 4 }} />
            </view>
          ),
        },
        {
          title: 'justifyContent',
          properties: ['justifyContent'],
          render: () => (
            <view style={{
              display: 'flex',
              flexDirection: 'row',
              justifyContent: 'space-between',
              width: 140,
              backgroundColor: p.panel2,
              borderRadius: 8,
            }}>
              <view style={{ width: 20, height: 20, backgroundColor: p.accent, borderRadius: 4 }} />
              <view style={{ width: 20, height: 20, backgroundColor: p.blue, borderRadius: 4 }} />
            </view>
          ),
        },
        {
          title: 'flexGrow',
          properties: ['flexGrow'],
          render: () => (
            <view style={{ display: 'flex', flexDirection: 'row', width: 140, gap: 4 }}>
              <view style={{ flexGrow: 1, height: 24, backgroundColor: p.accent, borderRadius: 4 }} />
              <view style={{ width: 24, height: 24, backgroundColor: p.blue, borderRadius: 4 }} />
            </view>
          ),
        },
        {
          title: 'flexShrink',
          properties: ['flexShrink'],
          render: () => (
            <view style={{ display: 'flex', flexDirection: 'row', width: 100, gap: 4 }}>
              <view style={{ width: 80, flexShrink: 2, height: 24, backgroundColor: p.accent, borderRadius: 4 }} />
              <view style={{ width: 80, flexShrink: 0, height: 24, backgroundColor: p.blue, borderRadius: 4 }} />
            </view>
          ),
        },
        {
          title: 'flexBasis',
          properties: ['flexBasis'],
          render: () => (
            <view style={{ display: 'flex', flexDirection: 'row', width: 140, gap: 4 }}>
              <view style={{ flexBasis: 60, height: 24, backgroundColor: p.accent, borderRadius: 4 }} />
              <view style={{ flexGrow: 1, height: 24, backgroundColor: p.blue, borderRadius: 4 }} />
            </view>
          ),
        },
        {
          title: 'alignSelf',
          properties: ['alignSelf'],
          render: () => (
            <view style={{
              display: 'flex',
              flexDirection: 'row',
              alignItems: 'flex-start',
              gap: 6,
              height: 56,
              backgroundColor: p.panel2,
              borderRadius: 8,
            }}>
              <view style={{ width: 20, height: 20, backgroundColor: p.muted, borderRadius: 4 }} />
              <view style={{ width: 20, height: 36, alignSelf: 'flex-end', backgroundColor: p.accent, borderRadius: 4 }} />
            </view>
          ),
        },
        {
          title: 'alignContent',
          properties: ['alignContent'],
          render: () => (
            <view style={{
              display: 'flex',
              flexWrap: 'wrap',
              alignContent: 'space-between',
              width: 100,
              height: 72,
              gap: 4,
              backgroundColor: p.panel2,
              borderRadius: 8,
            }}>
              <view style={{ width: 40, height: 20, backgroundColor: p.accent, borderRadius: 4 }} />
              <view style={{ width: 40, height: 20, backgroundColor: p.blue, borderRadius: 4 }} />
              <view style={{ width: 40, height: 20, backgroundColor: p.violet, borderRadius: 4 }} />
              <view style={{ width: 40, height: 20, backgroundColor: p.accent, borderRadius: 4 }} />
            </view>
          ),
        },
        {
          title: 'zIndex',
          properties: ['zIndex'],
          render: () => (
            <view style={{ display: 'flex', flexDirection: 'row', width: 100, height: 40 }}>
              <view style={{ width: 56, height: 32, backgroundColor: p.panel3, zIndex: 1, borderRadius: 6 }} />
              <view style={{ width: 56, height: 32, backgroundColor: p.accent, zIndex: 2, marginLeft: -24, borderRadius: 6 }} />
            </view>
          ),
        },
        {
          title: 'gridTemplateColumns',
          properties: ['gridTemplateColumns'],
          render: () => (
            <view style={{
              display: 'grid',
              gridTemplateColumns: ['1fr', '1fr'],
              gap: 6,
              width: 140,
              backgroundColor: p.panel2,
              padding: 6,
              borderRadius: 8,
            }}>
              <view style={{ height: 24, backgroundColor: p.accent, borderRadius: 4 }} />
              <view style={{ height: 24, backgroundColor: p.blue, borderRadius: 4 }} />
            </view>
          ),
        },
        {
          title: 'gridTemplateRows',
          properties: ['gridTemplateRows'],
          render: () => (
            <view style={{
              display: 'grid',
              gridTemplateRows: ['1fr', '1fr'],
              gap: 6,
              width: 100,
              height: 72,
              backgroundColor: p.panel2,
              padding: 6,
              borderRadius: 8,
            }}>
              <view style={{ backgroundColor: p.accent, borderRadius: 4 }} />
              <view style={{ backgroundColor: p.blue, borderRadius: 4 }} />
            </view>
          ),
        },
      ],
    },
    {
      title: 'Position & Overflow',
      accent: p.success,
      cards: [
        {
          title: 'position / top / left / right / bottom',
          properties: ['position', 'top', 'left', 'right', 'bottom'],
          note: 'absolute children pinned to corners',
          render: () => (
            <view style={{
              position: 'relative',
              width: 160,
              height: 80,
              backgroundColor: p.panel2,
              borderRadius: 8,
              borderWidth: 1,
              borderColor: p.line,
            }}>
              <view style={{ position: 'absolute', top: 8, left: 8, width: 28, height: 28, backgroundColor: p.accent, borderRadius: 6 }} />
              <view style={{ position: 'absolute', right: 8, bottom: 8, width: 28, height: 28, backgroundColor: p.accent2, borderRadius: 6 }} />
            </view>
          ),
        },
        {
          title: 'overflow',
          properties: ['overflow'],
          note: 'hidden clips the oversized child',
          render: () => (
            <view style={{
              width: 96,
              height: 56,
              overflow: 'hidden',
              backgroundColor: p.panel2,
              borderRadius: 8,
              borderWidth: 1,
              borderColor: p.line,
            }}>
              <view style={{ width: 160, height: 100, backgroundColor: p.accent, borderRadius: 6 }} />
            </view>
          ),
        },
      ],
    },
    {
      title: 'Text',
      accent: p.blue,
      cards: [
        {
          title: 'fontSize',
          properties: ['fontSize'],
          render: () => <text style={{ fontSize: 22, color: p.text }}>Sample</text>,
        },
        {
          title: 'fontFamily',
          properties: ['fontFamily'],
          render: () => <text style={{ fontFamily: 'Georgia, serif', color: p.text }}>Sample</text>,
        },
        {
          title: 'fontWeight',
          properties: ['fontWeight'],
          render: () => (
            <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
              <text style={{ fontWeight: 400, color: p.text }}>Regular 400</text>
              <text style={{ fontWeight: 600, color: p.text }}>Semibold 600</text>
              <text style={{ fontWeight: 700, color: p.text }}>Bold 700</text>
            </view>
          ),
        },
        {
          title: 'fontStyle',
          properties: ['fontStyle'],
          render: () => (
            <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
              <text style={{ fontStyle: 'normal', color: p.text }}>Upright</text>
              <text style={{ fontStyle: 'italic', color: p.text }}>Italic (synth)</text>
            </view>
          ),
        },
        {
          title: 'textDecoration',
          properties: ['textDecoration'],
          render: () => <text style={{ textDecoration: 'underline', color: p.text }}>Sample</text>,
        },
        {
          title: 'color',
          properties: ['color'],
          render: () => <text style={{ color: p.accent }}>Sample</text>,
        },
        {
          title: 'maxLines / textOverflow',
          properties: ['maxLines', 'textOverflow'],
          note: 'clamp to 2 lines with ellipsis',
          render: () => (
            <view style={{ width: 168 }}>
              <text style={{ color: p.text, fontSize: 13, maxLines: 2, textOverflow: 'ellipsis' }}>
                This caption runs long on purpose so the renderer clamps it to two lines and trails an ellipsis.
              </text>
            </view>
          ),
        },
        {
          title: 'defaultColor / defaultFontFamily / defaultFontSize / defaultFontWeight',
          properties: ['defaultColor', 'defaultFontFamily', 'defaultFontSize', 'defaultFontWeight'],
          note: 'inherited text defaults',
          render: () => (
            <view style={{
              display: 'flex',
              flexDirection: 'column',
              gap: 6,
              padding: 10,
              backgroundColor: p.panel2,
              borderWidth: 1,
              borderColor: p.line,
              borderRadius: 8,
              defaultColor: p.accent2,
              defaultFontFamily: 'Georgia, serif',
              defaultFontSize: 18,
              defaultFontWeight: 700,
            }}>
              <text>Inherited text styles</text>
              <text>Second line inherits defaults</text>
            </view>
          ),
        },
      ],
    },
    {
      title: 'Motion',
      accent: p.accent,
      cards: [
        {
          title: 'transitionDuration / transitionTiming',
          properties: ['transitionDuration', 'transitionTiming'],
          note: 'hover to ease the color over 250ms',
          render: () => (
            <button style={{
              height: 40,
              paddingLeft: 16,
              paddingRight: 16,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: p.panel2,
              defaultColor: p.text,
              borderRadius: 10,
              borderWidth: 1,
              borderColor: p.line,
              transitionDuration: 250,
              transitionTiming: 'ease-out',
              ':hover': {
                backgroundColor: p.accent,
                defaultColor: p.black,
                borderColor: p.accent,
              },
            }}>
              Hover to ease
            </button>
          ),
        },
      ],
    },
    {
      title: 'Interaction & Elements',
      accent: p.accent2,
      cards: [
        {
          title: 'cursor',
          properties: ['cursor'],
          note: 'hover each tile — the pointer changes and the tile lights up',
          render: () => (
            <view style={{ display: 'flex', flexWrap: 'wrap', gap: 6, width: 168 }}>
              {(['pointer', 'grab', 'text', 'not-allowed'] as const).map((kind) => (
                <view style={{
                  width: 78,
                  height: 30,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  cursor: kind,
                  backgroundColor: p.panel2,
                  defaultColor: p.text,
                  borderRadius: 8,
                  borderWidth: 1,
                  borderColor: p.line,
                  transitionDuration: 150,
                  transitionTiming: 'ease-out',
                  ':hover': {
                    backgroundColor: p.accent,
                    defaultColor: p.black,
                    borderColor: p.accent,
                  },
                }}>
                  <text style={{ fontSize: 11 }}>{kind}</text>
                </view>
              ))}
            </view>
          ),
        },
        {
          title: ':hover',
          properties: [],
          render: () => (
            <button style={{
              height: 36,
              paddingLeft: 14,
              paddingRight: 14,
              backgroundColor: p.panel2,
              defaultColor: p.text,
              borderRadius: 10,
              borderWidth: 1,
              borderColor: p.line,
              ':hover': {
                backgroundColor: p.accent,
                defaultColor: p.black,
                borderColor: p.accent,
              },
            }}>
              Hover me
            </button>
          ),
        },
        {
          title: ':active',
          properties: [],
          render: () => (
            <button style={{
              height: 36,
              paddingLeft: 14,
              paddingRight: 14,
              backgroundColor: p.panel2,
              defaultColor: p.text,
              borderRadius: 10,
              borderWidth: 1,
              borderColor: p.line,
              ':active': {
                backgroundColor: p.accent2,
                defaultColor: p.black,
                borderColor: p.accent2,
              },
            }}>
              Press me
            </button>
          ),
        },
        {
          title: ':focus',
          properties: [],
          render: () => <text-input value="Focus me" style={inputStyle(p)} />,
        },
        {
          title: 'scroll-view',
          properties: [],
          render: () => (
            <scroll-view style={{
              width: 168,
              height: 72,
              backgroundColor: p.panel2,
              borderWidth: 1,
              borderColor: p.line,
              borderRadius: 8,
              padding: 8,
            }}>
              <view style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                {([1, 2, 3, 4, 5, 6] as const).map((n) => (
                  <text style={{ color: p.text, fontSize: 12 }}>{`Line ${n}`}</text>
                ))}
              </view>
            </scroll-view>
          ),
        },
        {
          title: 'nested scroll (chaining)',
          properties: [],
          render: () => (
            <scroll-view style={{
              width: 180,
              height: 120,
              backgroundColor: p.panel,
              borderWidth: 1,
              borderColor: p.accent,
              borderRadius: 8,
              padding: 6,
            }}>
              <view style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                <text style={{ color: p.muted, fontSize: 11 }}>Outer — scroll past inner edge</text>
                <scroll-view style={{
                  width: 160,
                  height: 64,
                  backgroundColor: p.panel2,
                  borderWidth: 1,
                  borderColor: p.line,
                  borderRadius: 6,
                  padding: 6,
                }}>
                  <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                    {(['A', 'B', 'C', 'D', 'E'] as const).map((c) => (
                      <text style={{ color: p.text, fontSize: 11 }}>{`Inner ${c}`}</text>
                    ))}
                  </view>
                </scroll-view>
                <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                  {([1, 2, 3, 4] as const).map((n) => (
                    <text style={{ color: p.text, fontSize: 11 }}>{`Outer tail ${n}`}</text>
                  ))}
                </view>
              </view>
            </scroll-view>
          ),
        },
        {
          title: 'text-input',
          properties: [],
          render: () => <text-input placeholder="Type here" value="" style={inputStyle(p)} />,
        },
        {
          title: 'button',
          properties: [],
          render: () => (
            <button style={{
              height: 36,
              paddingLeft: 14,
              paddingRight: 14,
              backgroundColor: p.blue,
              defaultColor: p.black,
              borderRadius: 10,
              borderWidth: 1,
              borderColor: p.blue,
            }}>
              Click
            </button>
          ),
        },
        {
          // ADR-0108: selectability mirrors CSS `user-select` — view / text are
          // selectable by the element-kind default (no declaration needed);
          // `user-select: none` opts a subtree out.
          title: 'user-select',
          properties: [],
          note: 'view/text 既定選択可・user-select:none で除外',
          render: () => (
            <view style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              <view style={{
                padding: 8,
                backgroundColor: p.panel2,
                borderRadius: 8,
                borderWidth: 1,
                borderColor: p.line,
              }}>
                <text style={{ color: p.text, fontSize: 12 }}>既定で選択できる（宣言なし）</text>
              </view>
              <view user-select="none" style={{
                padding: 8,
                backgroundColor: p.panel2,
                borderRadius: 8,
                borderWidth: 1,
                borderColor: p.line,
              }}>
                <text style={{ color: p.muted, fontSize: 12 }}>user-select: none で選択不可</text>
              </view>
            </view>
          ),
        },
      ],
    },
    {
      title: 'Responsive',
      accent: p.success,
      cards: [
        {
          // ADR-0081: viewport variants compile to real `@media (min-width: …)`
          // rules in the DOM Renderer. Renderer-feature card (no catalog patchKey).
          title: '@media / styleVariants',
          properties: [],
          note: 'ウィンドウ幅を変えると一致する帯だけ点灯。DOM では本物の @media ルール（DevTools の <style data-tsubame-variant>）。',
          render: () => <MediaTiles colors={p} />,
        },
      ],
    },
  ];
}

// Future CSS candidates not yet in style_tags.json — shown as static reference.
// `box-shadow` graduated to a live Visual card once ADR-0095 (#252) shipped it to
// the catalog; `cursor` likewise already graduated to the live Interaction section.
export const ROADMAP: readonly (readonly [string, string])[] = [
  ['transform', '2D/3D transforms (translate, scale, rotate)'],
  ['textAlign', 'Horizontal text alignment'],
  ['lineHeight', 'Line box height for text'],
  ['letterSpacing', 'Tracking between glyphs'],
  ['outline', 'Focus ring outside border box'],
];

/** Catalog patchKeys with a live POP card, derived from the section descriptors. */
export const GALLERY_LIVE_PROPERTIES: readonly string[] = buildSections(palette(DEFAULT_THEME, DEFAULT_ACCENT))
  .flatMap((section) => section.cards)
  .flatMap((card) => card.properties);

/** Property names shown as static roadmap reference (not yet in the catalog). */
export const GALLERY_ROADMAP_PROPERTIES: readonly string[] = ROADMAP.map(([name]) => name);
