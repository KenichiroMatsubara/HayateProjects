import type { HayateCssStyle } from '@tsubame/renderer-protocol';
import { COLORS, inputStyle } from './theme';

interface PropertySampleProps {
  name: string;
  children: unknown;
}

function PropertySample(props: PropertySampleProps) {
  return (
    <view style={{
      display: 'flex',
      flexDirection: 'column',
      gap: 8,
      minWidth: 180,
      padding: 12,
      backgroundColor: COLORS.panel,
      borderRadius: 8,
      borderWidth: 1,
      borderColor: COLORS.line,
    }}>
      <text style={{ color: COLORS.muted, fontSize: 12 }}>{props.name}</text>
      {props.children}
    </view>
  );
}

function Section(props: { title: string; children: unknown }) {
  return (
    <view style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      <text style={{ color: COLORS.ink, fontSize: 18 }}>{props.title}</text>
      <view style={{ display: 'flex', flexWrap: 'wrap', gap: 12 }}>
        {props.children}
      </view>
    </view>
  );
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
      borderRadius: 6,
      ...props.style,
    }}>
      <text style={{ color: COLORS.text, fontSize: 12 }}>{props.label}</text>
    </view>
  );
}

function VisualSection() {
  return (
    <Section title="Visual">
      <PropertySample name="backgroundColor">
        <SampleBox label="Sample" style={{ backgroundColor: '#4fd1c5' }} />
      </PropertySample>
      <PropertySample name="opacity">
        <SampleBox label="Sample" style={{ opacity: 0.45 }} />
      </PropertySample>
      <PropertySample name="borderRadius">
        <SampleBox label="Sample" style={{ borderRadius: 16 }} />
      </PropertySample>
      <PropertySample name="borderWidth">
        <SampleBox label="Sample" style={{ borderWidth: 3, borderColor: COLORS.accent }} />
      </PropertySample>
      <PropertySample name="borderColor">
        <SampleBox label="Sample" style={{ borderWidth: 2, borderColor: COLORS.violet }} />
      </PropertySample>
    </Section>
  );
}

function SizingSection() {
  const keys = ['width', 'height', 'minWidth', 'minHeight', 'maxWidth', 'maxHeight'] as const;
  const styles: Record<typeof keys[number], HayateCssStyle> = {
    width: { width: 140 },
    height: { height: 72 },
    minWidth: { minWidth: 120, width: 80 },
    minHeight: { minHeight: 64, height: 40 },
    maxWidth: { maxWidth: 90, width: 140 },
    maxHeight: { maxHeight: 40, height: 72 },
  };
  return (
    <Section title="Sizing">
      {keys.map((key) => (
        <PropertySample name={key}>
          <SampleBox label="Sample" style={styles[key]} />
        </PropertySample>
      ))}
    </Section>
  );
}

function SpacingSection() {
  const paddingKeys = ['padding', 'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft'] as const;
  const marginKeys = ['margin', 'marginTop', 'marginRight', 'marginBottom', 'marginLeft'] as const;
  return (
    <Section title="Spacing">
      {paddingKeys.map((key) => (
        <PropertySample name={key}>
          <view style={{
            backgroundColor: COLORS.panel2,
            borderWidth: 1,
            borderColor: COLORS.line,
            [key]: 14,
          }}>
            <view style={{ backgroundColor: COLORS.accent, height: 28, width: 80 }} />
          </view>
        </PropertySample>
      ))}
      {marginKeys.map((key) => (
        <PropertySample name={key}>
          <view style={{ backgroundColor: COLORS.black, padding: 4 }}>
            <view style={{
              backgroundColor: COLORS.panel2,
              borderWidth: 1,
              borderColor: COLORS.line,
              height: 28,
              width: 80,
              [key]: 10,
            }} />
          </view>
        </PropertySample>
      ))}
      <PropertySample name="gap">
        <view style={{
          display: 'flex',
          flexDirection: 'row',
          gap: 16,
          backgroundColor: COLORS.panel2,
          padding: 8,
          borderWidth: 1,
          borderColor: COLORS.line,
        }}>
          <view style={{ width: 36, height: 24, backgroundColor: COLORS.accent }} />
          <view style={{ width: 36, height: 24, backgroundColor: COLORS.blue }} />
        </view>
      </PropertySample>
    </Section>
  );
}

function FlexLayoutSection() {
  return (
    <Section title="Flex Layout">
      <PropertySample name="display">
        <view style={{ display: 'flex', flexDirection: 'row', gap: 6 }}>
          <view style={{ width: 24, height: 24, backgroundColor: COLORS.accent }} />
          <view style={{ width: 24, height: 24, backgroundColor: COLORS.blue }} />
        </view>
      </PropertySample>
      <PropertySample name="flexDirection">
        <view style={{ display: 'flex', flexDirection: 'column', gap: 6, height: 72 }}>
          <view style={{ width: 48, height: 16, backgroundColor: COLORS.accent }} />
          <view style={{ width: 48, height: 16, backgroundColor: COLORS.blue }} />
        </view>
      </PropertySample>
      <PropertySample name="flexWrap">
        <view style={{
          display: 'flex',
          flexWrap: 'wrap',
          width: 120,
          gap: 4,
        }}>
          <view style={{ width: 48, height: 20, backgroundColor: COLORS.accent }} />
          <view style={{ width: 48, height: 20, backgroundColor: COLORS.blue }} />
          <view style={{ width: 48, height: 20, backgroundColor: COLORS.violet }} />
        </view>
      </PropertySample>
      <PropertySample name="alignItems">
        <view style={{
          display: 'flex',
          flexDirection: 'row',
          alignItems: 'center',
          gap: 6,
          height: 56,
          backgroundColor: COLORS.panel2,
        }}>
          <view style={{ width: 20, height: 20, backgroundColor: COLORS.accent }} />
          <view style={{ width: 20, height: 36, backgroundColor: COLORS.blue }} />
        </view>
      </PropertySample>
      <PropertySample name="justifyContent">
        <view style={{
          display: 'flex',
          flexDirection: 'row',
          justifyContent: 'space-between',
          width: 140,
          backgroundColor: COLORS.panel2,
        }}>
          <view style={{ width: 20, height: 20, backgroundColor: COLORS.accent }} />
          <view style={{ width: 20, height: 20, backgroundColor: COLORS.blue }} />
        </view>
      </PropertySample>
      <PropertySample name="flexGrow">
        <view style={{ display: 'flex', flexDirection: 'row', width: 140, gap: 4 }}>
          <view style={{ flexGrow: 1, height: 24, backgroundColor: COLORS.accent }} />
          <view style={{ width: 24, height: 24, backgroundColor: COLORS.blue }} />
        </view>
      </PropertySample>
      <PropertySample name="flexShrink">
        <view style={{ display: 'flex', flexDirection: 'row', width: 100, gap: 4 }}>
          <view style={{ width: 80, flexShrink: 2, height: 24, backgroundColor: COLORS.accent }} />
          <view style={{ width: 80, flexShrink: 0, height: 24, backgroundColor: COLORS.blue }} />
        </view>
      </PropertySample>
      <PropertySample name="flexBasis">
        <view style={{ display: 'flex', flexDirection: 'row', width: 140, gap: 4 }}>
          <view style={{ flexBasis: 60, height: 24, backgroundColor: COLORS.accent }} />
          <view style={{ flexGrow: 1, height: 24, backgroundColor: COLORS.blue }} />
        </view>
      </PropertySample>
      <PropertySample name="alignSelf">
        <view style={{
          display: 'flex',
          flexDirection: 'row',
          alignItems: 'flex-start',
          gap: 6,
          height: 56,
          backgroundColor: COLORS.panel2,
        }}>
          <view style={{ width: 20, height: 20, backgroundColor: COLORS.muted }} />
          <view style={{ width: 20, height: 36, alignSelf: 'flex-end', backgroundColor: COLORS.accent }} />
        </view>
      </PropertySample>
      <PropertySample name="alignContent">
        <view style={{
          display: 'flex',
          flexWrap: 'wrap',
          alignContent: 'space-between',
          width: 100,
          height: 72,
          gap: 4,
          backgroundColor: COLORS.panel2,
        }}>
          <view style={{ width: 40, height: 20, backgroundColor: COLORS.accent }} />
          <view style={{ width: 40, height: 20, backgroundColor: COLORS.blue }} />
          <view style={{ width: 40, height: 20, backgroundColor: COLORS.violet }} />
          <view style={{ width: 40, height: 20, backgroundColor: COLORS.accent }} />
        </view>
      </PropertySample>
      <PropertySample name="zIndex">
        <view style={{ display: 'flex', flexDirection: 'row', width: 100, height: 40 }}>
          <view style={{
            width: 56,
            height: 32,
            backgroundColor: COLORS.panel3,
            zIndex: 1,
          }} />
          <view style={{
            width: 56,
            height: 32,
            backgroundColor: COLORS.accent,
            zIndex: 2,
            marginLeft: -24,
          }} />
        </view>
      </PropertySample>
    </Section>
  );
}

function AdvancedSection() {
  return (
    <Section title="Advanced">
      <PropertySample name="gridTemplateColumns">
        <view style={{
          display: 'grid',
          gridTemplateColumns: ['1fr', '1fr'],
          gap: 6,
          width: 140,
          backgroundColor: COLORS.panel2,
          padding: 6,
        }}>
          <view style={{ height: 24, backgroundColor: COLORS.accent }} />
          <view style={{ height: 24, backgroundColor: COLORS.blue }} />
        </view>
      </PropertySample>
      <PropertySample name="gridTemplateRows">
        <view style={{
          display: 'grid',
          gridTemplateRows: ['1fr', '1fr'],
          gap: 6,
          width: 100,
          height: 72,
          backgroundColor: COLORS.panel2,
          padding: 6,
        }}>
          <view style={{ backgroundColor: COLORS.accent }} />
          <view style={{ backgroundColor: COLORS.blue }} />
        </view>
      </PropertySample>
      <PropertySample name="defaultColor / defaultFontFamily / defaultFontSize / defaultFontWeight">
        <view style={{
          display: 'flex',
          flexDirection: 'column',
          gap: 6,
          padding: 10,
          backgroundColor: COLORS.panel2,
          borderWidth: 1,
          borderColor: COLORS.line,
          defaultColor: COLORS.accent2,
          defaultFontFamily: 'Georgia, serif',
          defaultFontSize: 18,
          defaultFontWeight: 700,
        }}>
          <text>Inherited text styles</text>
          <text>Second line inherits defaults</text>
        </view>
      </PropertySample>
    </Section>
  );
}

function TextSection() {
  return (
    <Section title="Text">
      <PropertySample name="fontSize">
        <text style={{ fontSize: 22, color: COLORS.text }}>Sample</text>
      </PropertySample>
      <PropertySample name="fontFamily">
        <text style={{ fontFamily: 'Georgia, serif', color: COLORS.text }}>Sample</text>
      </PropertySample>
      <PropertySample name="fontWeight">
        <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
          <text style={{ fontWeight: 400, color: COLORS.text }}>Regular 400</text>
          <text style={{ fontWeight: 600, color: COLORS.text }}>Semibold 600</text>
          <text style={{ fontWeight: 700, color: COLORS.text }}>Bold 700</text>
        </view>
      </PropertySample>
      <PropertySample name="fontStyle">
        <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
          <text style={{ fontStyle: 'normal', color: COLORS.text }}>Upright</text>
          <text style={{ fontStyle: 'italic', color: COLORS.text }}>Italic (synth)</text>
        </view>
      </PropertySample>
      <PropertySample name="textDecoration">
        <text style={{ textDecoration: 'underline', color: COLORS.text }}>Sample</text>
      </PropertySample>
      <PropertySample name="color">
        <text style={{ color: COLORS.accent }}>Sample</text>
      </PropertySample>
    </Section>
  );
}

function InteractionSection() {
  return (
    <Section title="Interaction States">
      <PropertySample name=":hover">
        <button style={{
          height: 36,
          paddingLeft: 14,
          paddingRight: 14,
          backgroundColor: COLORS.panel2,
          defaultColor: COLORS.text,
          borderRadius: 8,
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
      </PropertySample>
      <PropertySample name=":active">
        <button style={{
          height: 36,
          paddingLeft: 14,
          paddingRight: 14,
          backgroundColor: COLORS.panel2,
          defaultColor: COLORS.text,
          borderRadius: 8,
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
      </PropertySample>
      <PropertySample name=":focus">
        <text-input
          value="Focus me"
          style={inputStyle}
        />
      </PropertySample>
      <PropertySample name="scroll-view">
        <scroll-view style={{
          width: 160,
          height: 72,
          backgroundColor: COLORS.panel2,
          borderWidth: 1,
          borderColor: COLORS.line,
          padding: 8,
        }}>
          <view style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            <text style={{ color: COLORS.text, fontSize: 12 }}>Line 1</text>
            <text style={{ color: COLORS.text, fontSize: 12 }}>Line 2</text>
            <text style={{ color: COLORS.text, fontSize: 12 }}>Line 3</text>
            <text style={{ color: COLORS.text, fontSize: 12 }}>Line 4</text>
            <text style={{ color: COLORS.text, fontSize: 12 }}>Line 5</text>
            <text style={{ color: COLORS.text, fontSize: 12 }}>Line 6</text>
          </view>
        </scroll-view>
      </PropertySample>
      <PropertySample name="nested scroll (chaining)">
        <scroll-view style={{
          width: 180,
          height: 120,
          backgroundColor: COLORS.panel,
          borderWidth: 1,
          borderColor: COLORS.accent,
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
              padding: 6,
            }}>
              <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                <text style={{ color: COLORS.text, fontSize: 11 }}>Inner A</text>
                <text style={{ color: COLORS.text, fontSize: 11 }}>Inner B</text>
                <text style={{ color: COLORS.text, fontSize: 11 }}>Inner C</text>
                <text style={{ color: COLORS.text, fontSize: 11 }}>Inner D</text>
                <text style={{ color: COLORS.text, fontSize: 11 }}>Inner E</text>
              </view>
            </scroll-view>
            <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
              <text style={{ color: COLORS.text, fontSize: 11 }}>Outer tail 1</text>
              <text style={{ color: COLORS.text, fontSize: 11 }}>Outer tail 2</text>
              <text style={{ color: COLORS.text, fontSize: 11 }}>Outer tail 3</text>
              <text style={{ color: COLORS.text, fontSize: 11 }}>Outer tail 4</text>
            </view>
          </view>
        </scroll-view>
      </PropertySample>
      <PropertySample name="text-input">
        <text-input
          placeholder="Type here"
          value=""
          style={inputStyle}
        />
      </PropertySample>
      <PropertySample name="button">
        <button style={{
          height: 36,
          paddingLeft: 14,
          paddingRight: 14,
          backgroundColor: COLORS.blue,
          defaultColor: COLORS.black,
          borderRadius: 8,
          borderWidth: 1,
          borderColor: COLORS.blue,
        }}>
          Click
        </button>
      </PropertySample>
    </Section>
  );
}

const ROADMAP_PROPERTIES = [
  ['transform', '2D/3D transforms (translate, scale, rotate)'],
  ['boxShadow', 'Drop shadows and elevation'],
  ['cursor', 'Pointer cursor styles'],
  ['textAlign', 'Horizontal text alignment'],
  ['lineHeight', 'Line box height for text'],
  ['letterSpacing', 'Tracking between glyphs'],
  ['outline', 'Focus ring outside border box'],
] as const;

function RoadmapSection() {
  return (
    <Section title="Roadmap / 未実装">
      <view style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 10,
        width: '100%',
        padding: 16,
        backgroundColor: COLORS.panel,
        borderRadius: 8,
        borderWidth: 1,
        borderColor: COLORS.line,
      }}>
        <text style={{ color: COLORS.muted, fontSize: 13 }}>
          Future CSS candidates not yet in style_tags.json — shown as static reference only.
        </text>
        {ROADMAP_PROPERTIES.map(([name, description]) => (
          <view style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            <text style={{ color: COLORS.accent2, fontSize: 14 }}>{name}</text>
            <text style={{ color: COLORS.quiet, fontSize: 12 }}>{description}</text>
          </view>
        ))}
      </view>
    </Section>
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
      <view style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        <text style={{ color: COLORS.ink, fontSize: 22 }}>CSS Gallery</text>
        <text style={{ color: COLORS.muted, fontSize: 13 }}>
          HayateStyle catalog — all 40 properties from style_tags.json
        </text>
      </view>
      <VisualSection />
      <SizingSection />
      <SpacingSection />
      <FlexLayoutSection />
      <AdvancedSection />
      <TextSection />
      <InteractionSection />
      <RoadmapSection />
    </scroll-view>
  );
}
