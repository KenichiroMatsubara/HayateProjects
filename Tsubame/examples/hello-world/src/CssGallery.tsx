import type { HayateCssStyle } from '@tsubame/renderer-protocol';
import { COLORS, inputStyle } from './theme';

// ─────────────────────────────────────────────────────────────────────────────
// Data-driven CSS Gallery (issue #246)
//
// Each Hayate CSS property gets a POP card whose body renders the property
// *actually in effect*. The gallery is described as data so the live coverage
// is inspectable: `GALLERY_LIVE_PROPERTIES` is derived from the same card
// descriptors that render on screen, and css-gallery.test.ts asserts it covers
// every property in the protocol catalog. Card titles use the catalog patchKey
// so the chip you read matches the key you'd write.
// ─────────────────────────────────────────────────────────────────────────────

interface GalleryCard {
  /** Header chip text. Usually the catalog patchKey it demonstrates. */
  title: string;
  /** Catalog patchKeys this card shows live. Empty for renderer-feature cards. */
  properties: readonly string[];
  /** Optional caption under the live body. */
  note?: string;
  render: () => unknown;
}

interface GallerySection {
  title: string;
  /** Playful per-section accent used on the rail, chip dot, and title. */
  accent: string;
  cards: readonly GalleryCard[];
}

function SampleBox(props: { label: string; style: HayateCssStyle }) {
  return (
    <view style={{
      width: 120,
      height: 56,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: COLORS.panel2,
      borderWidth: 1,
      borderColor: COLORS.line,
      borderRadius: 10,
      ...props.style,
    }}>
      <text style={{ color: COLORS.text, fontSize: 12 }}>{props.label}</text>
    </view>
  );
}

const SECTIONS: readonly GallerySection[] = [
  {
    title: 'Visual',
    accent: COLORS.accent,
    cards: [
      {
        title: 'backgroundColor',
        properties: ['backgroundColor'],
        render: () => <SampleBox label="Sample" style={{ backgroundColor: '#4fd1c5' }} />,
      },
      {
        title: 'opacity',
        properties: ['opacity'],
        render: () => <SampleBox label="0.45" style={{ opacity: 0.45 }} />,
      },
      {
        title: 'borderRadius',
        properties: ['borderRadius'],
        render: () => <SampleBox label="r16" style={{ borderRadius: 16 }} />,
      },
      {
        title: 'borderWidth',
        properties: ['borderWidth'],
        render: () => <SampleBox label="3px" style={{ borderWidth: 3, borderColor: COLORS.accent }} />,
      },
      {
        title: 'borderColor',
        properties: ['borderColor'],
        render: () => <SampleBox label="violet" style={{ borderWidth: 2, borderColor: COLORS.violet }} />,
      },
      {
        title: 'borderStyle',
        properties: ['borderStyle'],
        note: 'solid / dashed',
        render: () => (
          <view style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            <SampleBox label="solid" style={{ borderWidth: 2, borderStyle: 'solid', borderColor: COLORS.accent }} />
            <SampleBox label="dashed" style={{ borderWidth: 2, borderStyle: 'dashed', borderColor: COLORS.accent2 }} />
          </view>
        ),
      },
    ],
  },
  {
    title: 'Sizing',
    accent: COLORS.blue,
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
      render: () => <SampleBox label="Sample" style={style} />,
    })),
  },
  {
    title: 'Spacing',
    accent: COLORS.violet,
    cards: [
      ...(['padding', 'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft'] as const).map((key) => ({
        title: key,
        properties: [key] as readonly string[],
        render: () => (
          <view style={{
            backgroundColor: COLORS.panel2,
            borderWidth: 1,
            borderColor: COLORS.line,
            borderRadius: 8,
            [key]: 14,
          }}>
            <view style={{ backgroundColor: COLORS.accent, height: 28, width: 80, borderRadius: 6 }} />
          </view>
        ),
      })),
      ...(['margin', 'marginTop', 'marginRight', 'marginBottom', 'marginLeft'] as const).map((key) => ({
        title: key,
        properties: [key] as readonly string[],
        render: () => (
          <view style={{ backgroundColor: COLORS.black, padding: 4, borderRadius: 8 }}>
            <view style={{
              backgroundColor: COLORS.panel2,
              borderWidth: 1,
              borderColor: COLORS.line,
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
            backgroundColor: COLORS.panel2,
            padding: 8,
            borderRadius: 8,
            borderWidth: 1,
            borderColor: COLORS.line,
          }}>
            <view style={{ width: 36, height: 24, backgroundColor: COLORS.accent, borderRadius: 6 }} />
            <view style={{ width: 36, height: 24, backgroundColor: COLORS.blue, borderRadius: 6 }} />
          </view>
        ),
      },
    ],
  },
  {
    title: 'Flex & Grid',
    accent: COLORS.accent2,
    cards: [
      {
        title: 'display',
        properties: ['display'],
        render: () => (
          <view style={{ display: 'flex', flexDirection: 'row', gap: 6 }}>
            <view style={{ width: 24, height: 24, backgroundColor: COLORS.accent, borderRadius: 6 }} />
            <view style={{ width: 24, height: 24, backgroundColor: COLORS.blue, borderRadius: 6 }} />
          </view>
        ),
      },
      {
        title: 'flexDirection',
        properties: ['flexDirection'],
        render: () => (
          <view style={{ display: 'flex', flexDirection: 'column', gap: 6, height: 72 }}>
            <view style={{ width: 48, height: 16, backgroundColor: COLORS.accent, borderRadius: 4 }} />
            <view style={{ width: 48, height: 16, backgroundColor: COLORS.blue, borderRadius: 4 }} />
          </view>
        ),
      },
      {
        title: 'flexWrap',
        properties: ['flexWrap'],
        render: () => (
          <view style={{ display: 'flex', flexWrap: 'wrap', width: 120, gap: 4 }}>
            <view style={{ width: 48, height: 20, backgroundColor: COLORS.accent, borderRadius: 4 }} />
            <view style={{ width: 48, height: 20, backgroundColor: COLORS.blue, borderRadius: 4 }} />
            <view style={{ width: 48, height: 20, backgroundColor: COLORS.violet, borderRadius: 4 }} />
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
            backgroundColor: COLORS.panel2,
            borderRadius: 8,
          }}>
            <view style={{ width: 20, height: 20, backgroundColor: COLORS.accent, borderRadius: 4 }} />
            <view style={{ width: 20, height: 36, backgroundColor: COLORS.blue, borderRadius: 4 }} />
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
            backgroundColor: COLORS.panel2,
            borderRadius: 8,
          }}>
            <view style={{ width: 20, height: 20, backgroundColor: COLORS.accent, borderRadius: 4 }} />
            <view style={{ width: 20, height: 20, backgroundColor: COLORS.blue, borderRadius: 4 }} />
          </view>
        ),
      },
      {
        title: 'flexGrow',
        properties: ['flexGrow'],
        render: () => (
          <view style={{ display: 'flex', flexDirection: 'row', width: 140, gap: 4 }}>
            <view style={{ flexGrow: 1, height: 24, backgroundColor: COLORS.accent, borderRadius: 4 }} />
            <view style={{ width: 24, height: 24, backgroundColor: COLORS.blue, borderRadius: 4 }} />
          </view>
        ),
      },
      {
        title: 'flexShrink',
        properties: ['flexShrink'],
        render: () => (
          <view style={{ display: 'flex', flexDirection: 'row', width: 100, gap: 4 }}>
            <view style={{ width: 80, flexShrink: 2, height: 24, backgroundColor: COLORS.accent, borderRadius: 4 }} />
            <view style={{ width: 80, flexShrink: 0, height: 24, backgroundColor: COLORS.blue, borderRadius: 4 }} />
          </view>
        ),
      },
      {
        title: 'flexBasis',
        properties: ['flexBasis'],
        render: () => (
          <view style={{ display: 'flex', flexDirection: 'row', width: 140, gap: 4 }}>
            <view style={{ flexBasis: 60, height: 24, backgroundColor: COLORS.accent, borderRadius: 4 }} />
            <view style={{ flexGrow: 1, height: 24, backgroundColor: COLORS.blue, borderRadius: 4 }} />
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
            backgroundColor: COLORS.panel2,
            borderRadius: 8,
          }}>
            <view style={{ width: 20, height: 20, backgroundColor: COLORS.muted, borderRadius: 4 }} />
            <view style={{ width: 20, height: 36, alignSelf: 'flex-end', backgroundColor: COLORS.accent, borderRadius: 4 }} />
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
            backgroundColor: COLORS.panel2,
            borderRadius: 8,
          }}>
            <view style={{ width: 40, height: 20, backgroundColor: COLORS.accent, borderRadius: 4 }} />
            <view style={{ width: 40, height: 20, backgroundColor: COLORS.blue, borderRadius: 4 }} />
            <view style={{ width: 40, height: 20, backgroundColor: COLORS.violet, borderRadius: 4 }} />
            <view style={{ width: 40, height: 20, backgroundColor: COLORS.accent, borderRadius: 4 }} />
          </view>
        ),
      },
      {
        title: 'zIndex',
        properties: ['zIndex'],
        render: () => (
          <view style={{ display: 'flex', flexDirection: 'row', width: 100, height: 40 }}>
            <view style={{ width: 56, height: 32, backgroundColor: COLORS.panel3, zIndex: 1, borderRadius: 6 }} />
            <view style={{ width: 56, height: 32, backgroundColor: COLORS.accent, zIndex: 2, marginLeft: -24, borderRadius: 6 }} />
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
            backgroundColor: COLORS.panel2,
            padding: 6,
            borderRadius: 8,
          }}>
            <view style={{ height: 24, backgroundColor: COLORS.accent, borderRadius: 4 }} />
            <view style={{ height: 24, backgroundColor: COLORS.blue, borderRadius: 4 }} />
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
            backgroundColor: COLORS.panel2,
            padding: 6,
            borderRadius: 8,
          }}>
            <view style={{ backgroundColor: COLORS.accent, borderRadius: 4 }} />
            <view style={{ backgroundColor: COLORS.blue, borderRadius: 4 }} />
          </view>
        ),
      },
    ],
  },
  {
    title: 'Position & Overflow',
    accent: COLORS.success,
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
            backgroundColor: COLORS.panel2,
            borderRadius: 8,
            borderWidth: 1,
            borderColor: COLORS.line,
          }}>
            <view style={{ position: 'absolute', top: 8, left: 8, width: 28, height: 28, backgroundColor: COLORS.accent, borderRadius: 6 }} />
            <view style={{ position: 'absolute', right: 8, bottom: 8, width: 28, height: 28, backgroundColor: COLORS.accent2, borderRadius: 6 }} />
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
            backgroundColor: COLORS.panel2,
            borderRadius: 8,
            borderWidth: 1,
            borderColor: COLORS.line,
          }}>
            <view style={{ width: 160, height: 100, backgroundColor: COLORS.accent, borderRadius: 6 }} />
          </view>
        ),
      },
    ],
  },
  {
    title: 'Text',
    accent: COLORS.blue,
    cards: [
      {
        title: 'fontSize',
        properties: ['fontSize'],
        render: () => <text style={{ fontSize: 22, color: COLORS.text }}>Sample</text>,
      },
      {
        title: 'fontFamily',
        properties: ['fontFamily'],
        render: () => <text style={{ fontFamily: 'Georgia, serif', color: COLORS.text }}>Sample</text>,
      },
      {
        title: 'fontWeight',
        properties: ['fontWeight'],
        render: () => (
          <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <text style={{ fontWeight: 400, color: COLORS.text }}>Regular 400</text>
            <text style={{ fontWeight: 600, color: COLORS.text }}>Semibold 600</text>
            <text style={{ fontWeight: 700, color: COLORS.text }}>Bold 700</text>
          </view>
        ),
      },
      {
        title: 'fontStyle',
        properties: ['fontStyle'],
        render: () => (
          <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <text style={{ fontStyle: 'normal', color: COLORS.text }}>Upright</text>
            <text style={{ fontStyle: 'italic', color: COLORS.text }}>Italic (synth)</text>
          </view>
        ),
      },
      {
        title: 'textDecoration',
        properties: ['textDecoration'],
        render: () => <text style={{ textDecoration: 'underline', color: COLORS.text }}>Sample</text>,
      },
      {
        title: 'color',
        properties: ['color'],
        render: () => <text style={{ color: COLORS.accent }}>Sample</text>,
      },
      {
        title: 'maxLines / textOverflow',
        properties: ['maxLines', 'textOverflow'],
        note: 'clamp to 2 lines with ellipsis',
        render: () => (
          <view style={{ width: 168 }}>
            <text style={{ color: COLORS.text, fontSize: 13, maxLines: 2, textOverflow: 'ellipsis' }}>
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
            backgroundColor: COLORS.panel2,
            borderWidth: 1,
            borderColor: COLORS.line,
            borderRadius: 8,
            defaultColor: COLORS.accent2,
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
    accent: COLORS.accent,
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
            backgroundColor: COLORS.panel2,
            defaultColor: COLORS.text,
            borderRadius: 10,
            borderWidth: 1,
            borderColor: COLORS.line,
            transitionDuration: 250,
            transitionTiming: 'ease-out',
            ':hover': {
              backgroundColor: COLORS.accent,
              defaultColor: COLORS.black,
              borderColor: COLORS.accent,
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
    accent: COLORS.accent2,
    cards: [
      {
        title: 'cursor',
        properties: ['cursor'],
        note: 'hover each tile to see the pointer',
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
                backgroundColor: COLORS.panel2,
                borderRadius: 8,
                borderWidth: 1,
                borderColor: COLORS.line,
              }}>
                <text style={{ color: COLORS.text, fontSize: 11 }}>{kind}</text>
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
            backgroundColor: COLORS.panel2,
            defaultColor: COLORS.text,
            borderRadius: 10,
            borderWidth: 1,
            borderColor: COLORS.line,
            ':hover': {
              backgroundColor: COLORS.accent,
              defaultColor: COLORS.black,
              borderColor: COLORS.accent,
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
            backgroundColor: COLORS.panel2,
            defaultColor: COLORS.text,
            borderRadius: 10,
            borderWidth: 1,
            borderColor: COLORS.line,
            ':active': {
              backgroundColor: COLORS.accent2,
              defaultColor: COLORS.black,
              borderColor: COLORS.accent2,
            },
          }}>
            Press me
          </button>
        ),
      },
      {
        title: ':focus',
        properties: [],
        render: () => <text-input value="Focus me" style={inputStyle} />,
      },
      {
        title: 'scroll-view',
        properties: [],
        render: () => (
          <scroll-view style={{
            width: 168,
            height: 72,
            backgroundColor: COLORS.panel2,
            borderWidth: 1,
            borderColor: COLORS.line,
            borderRadius: 8,
            padding: 8,
          }}>
            <view style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              {([1, 2, 3, 4, 5, 6] as const).map((n) => (
                <text style={{ color: COLORS.text, fontSize: 12 }}>{`Line ${n}`}</text>
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
            backgroundColor: COLORS.panel,
            borderWidth: 1,
            borderColor: COLORS.accent,
            borderRadius: 8,
            padding: 6,
          }}>
            <view style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <text style={{ color: COLORS.muted, fontSize: 11 }}>Outer — scroll past inner edge</text>
              <scroll-view style={{
                width: 160,
                height: 64,
                backgroundColor: COLORS.panel2,
                borderWidth: 1,
                borderColor: COLORS.line,
                borderRadius: 6,
                padding: 6,
              }}>
                <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                  {(['A', 'B', 'C', 'D', 'E'] as const).map((c) => (
                    <text style={{ color: COLORS.text, fontSize: 11 }}>{`Inner ${c}`}</text>
                  ))}
                </view>
              </scroll-view>
              <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                {([1, 2, 3, 4] as const).map((n) => (
                  <text style={{ color: COLORS.text, fontSize: 11 }}>{`Outer tail ${n}`}</text>
                ))}
              </view>
            </view>
          </scroll-view>
        ),
      },
      {
        title: 'text-input',
        properties: [],
        render: () => <text-input placeholder="Type here" value="" style={inputStyle} />,
      },
      {
        title: 'button',
        properties: [],
        render: () => (
          <button style={{
            height: 36,
            paddingLeft: 14,
            paddingRight: 14,
            backgroundColor: COLORS.blue,
            defaultColor: COLORS.black,
            borderRadius: 10,
            borderWidth: 1,
            borderColor: COLORS.blue,
          }}>
            Click
          </button>
        ),
      },
    ],
  },
];

// Future CSS candidates not yet in style_tags.json — shown as static reference.
// `box-shadow` is promoted to a live card once ADR-0095 (#252) ships it to the
// catalog; `cursor` already graduated to the live Interaction section.
const ROADMAP: readonly (readonly [string, string])[] = [
  ['boxShadow', 'Drop shadows and elevation — ADR-0095 (#252)'],
  ['transform', '2D/3D transforms (translate, scale, rotate)'],
  ['textAlign', 'Horizontal text alignment'],
  ['lineHeight', 'Line box height for text'],
  ['letterSpacing', 'Tracking between glyphs'],
  ['outline', 'Focus ring outside border box'],
];

/** Catalog patchKeys with a live POP card, derived from the rendered sections. */
export const GALLERY_LIVE_PROPERTIES: readonly string[] = SECTIONS
  .flatMap((section) => section.cards)
  .flatMap((card) => card.properties);

/** Property names shown as static roadmap reference (not yet in the catalog). */
export const GALLERY_ROADMAP_PROPERTIES: readonly string[] = ROADMAP.map(([name]) => name);

function PopCard(props: { title: string; accent: string; note?: string; children: unknown }) {
  return (
    <view style={{
      display: 'flex',
      flexDirection: 'column',
      gap: 12,
      minWidth: 200,
      maxWidth: 268,
      padding: 16,
      backgroundColor: COLORS.panel,
      borderRadius: 16,
      borderWidth: 1,
      borderColor: COLORS.line,
    }}>
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <view style={{ width: 10, height: 10, borderRadius: 6, backgroundColor: props.accent }} />
        <text style={{ color: props.accent, fontSize: 13, fontWeight: 600 }}>{props.title}</text>
      </view>
      <view style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        alignItems: 'flex-start',
        padding: 14,
        backgroundColor: COLORS.bg,
        borderRadius: 12,
        borderWidth: 1,
        borderColor: COLORS.line,
      }}>
        {props.children}
      </view>
      {props.note ? <text style={{ color: COLORS.quiet, fontSize: 11 }}>{props.note}</text> : null}
    </view>
  );
}

function SectionView(props: { section: GallerySection }) {
  return (
    <view style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <view style={{ width: 4, height: 22, borderRadius: 3, backgroundColor: props.section.accent }} />
        <text style={{ color: COLORS.ink, fontSize: 18, fontWeight: 600 }}>{props.section.title}</text>
      </view>
      <view style={{ display: 'flex', flexWrap: 'wrap', gap: 14 }}>
        {props.section.cards.map((card) => (
          <PopCard title={card.title} accent={props.section.accent} note={card.note}>
            {card.render()}
          </PopCard>
        ))}
      </view>
    </view>
  );
}

function RoadmapSection() {
  return (
    <view style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <view style={{ width: 4, height: 22, borderRadius: 3, backgroundColor: COLORS.quiet }} />
        <text style={{ color: COLORS.ink, fontSize: 18, fontWeight: 600 }}>Roadmap / 未実装</text>
      </view>
      <view style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 10,
        width: '100%',
        padding: 16,
        backgroundColor: COLORS.panel,
        borderRadius: 16,
        borderWidth: 1,
        borderColor: COLORS.line,
      }}>
        <text style={{ color: COLORS.muted, fontSize: 13 }}>
          Future CSS candidates not yet in style_tags.json — shown as static reference only.
        </text>
        {ROADMAP.map(([name, description]) => (
          <view style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            <text style={{ color: COLORS.accent2, fontSize: 14 }}>{name}</text>
            <text style={{ color: COLORS.quiet, fontSize: 12 }}>{description}</text>
          </view>
        ))}
      </view>
    </view>
  );
}

export function CssGallery() {
  return (
    <scroll-view style={{
      width: '100%',
      height: '100%',
      display: 'flex',
      flexDirection: 'column',
      gap: 28,
      paddingTop: 18,
      paddingLeft: 28,
      paddingRight: 28,
      paddingBottom: 28,
      backgroundColor: COLORS.bg,
    }}>
      <view style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <text style={{ color: COLORS.ink, fontSize: 24, fontWeight: 700 }}>CSS Gallery</text>
        <text style={{ color: COLORS.muted, fontSize: 13 }}>
          {`${GALLERY_LIVE_PROPERTIES.length} HayateStyle properties — each POP card renders the property live, in DOM and Canvas.`}
        </text>
      </view>
      {SECTIONS.map((section) => (
        <SectionView section={section} />
      ))}
      <RoadmapSection />
    </scroll-view>
  );
}
