import { type Palette } from '../theme';
import { ROADMAP, type GallerySection } from './sections';

function PopCard(props: { colors: Palette; title: string; accent: string; note?: string; children: unknown }) {
  return (
    <view style={{
      display: 'flex',
      flexDirection: 'column',
      gap: 12,
      minWidth: 200,
      maxWidth: 268,
      padding: 16,
      backgroundColor: props.colors.panel,
      borderRadius: 16,
      borderWidth: 1,
      borderColor: props.colors.line,
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
        backgroundColor: props.colors.bg,
        borderRadius: 12,
        borderWidth: 1,
        borderColor: props.colors.line,
      }}>
        {props.children}
      </view>
      {props.note ? <text style={{ color: props.colors.quiet, fontSize: 11 }}>{props.note}</text> : null}
    </view>
  );
}

export function SectionView(props: { colors: Palette; section: GallerySection }) {
  return (
    <view style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <view style={{ width: 4, height: 22, borderRadius: 3, backgroundColor: props.section.accent }} />
        <text style={{ color: props.colors.ink, fontSize: 18, fontWeight: 600 }}>{props.section.title}</text>
      </view>
      <view style={{ display: 'flex', flexWrap: 'wrap', gap: 14 }}>
        {props.section.cards.map((card) => (
          <PopCard colors={props.colors} title={card.title} accent={props.section.accent} note={card.note}>
            {card.render()}
          </PopCard>
        ))}
      </view>
    </view>
  );
}

export function RoadmapSection(props: { colors: Palette }) {
  return (
    <view style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <view style={{ width: 4, height: 22, borderRadius: 3, backgroundColor: props.colors.quiet }} />
        <text style={{ color: props.colors.ink, fontSize: 18, fontWeight: 600 }}>Roadmap / 未実装</text>
      </view>
      <view style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 10,
        width: '100%',
        padding: 16,
        backgroundColor: props.colors.panel,
        borderRadius: 16,
        borderWidth: 1,
        borderColor: props.colors.line,
      }}>
        <text style={{ color: props.colors.muted, fontSize: 13 }}>
          Future CSS candidates not yet in style_tags.json — shown as static reference only.
        </text>
        {ROADMAP.map(([name, description]) => (
          <view style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            <text style={{ color: props.colors.accent2, fontSize: 14 }}>{name}</text>
            <text style={{ color: props.colors.quiet, fontSize: 12 }}>{description}</text>
          </view>
        ))}
      </view>
    </view>
  );
}
