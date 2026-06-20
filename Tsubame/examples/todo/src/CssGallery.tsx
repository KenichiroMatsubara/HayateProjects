import { type Palette } from './theme';
import { buildSections, GALLERY_LIVE_PROPERTIES } from './gallery/sections';
import { RoadmapSection, SectionView } from './gallery/SectionView';

// データ駆動 CSS ギャラリーの公開面（issue #246）。セクション記述子（live 実証の
// 正本）は ./gallery/sections に、表示シェルの部品は ./gallery/SectionView にある。
// css-gallery.test.ts が参照する派生プロパティ一覧はここから再エクスポートする。
export { GALLERY_LIVE_PROPERTIES, GALLERY_ROADMAP_PROPERTIES } from './gallery/sections';

export function CssGallery(props: { colors: Palette }) {
  const sections = buildSections(props.colors);
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
      backgroundColor: props.colors.bg,
    }}>
      <view style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <text style={{ color: props.colors.ink, fontSize: 24, fontWeight: 700 }}>CSS Gallery</text>
        <text style={{ color: props.colors.muted, fontSize: 13 }}>
          {`${GALLERY_LIVE_PROPERTIES.length} HayateStyle properties — each POP card renders the property live, in DOM and Canvas.`}
        </text>
      </view>
      {sections.map((section) => (
        <SectionView colors={props.colors} section={section} />
      ))}
      <RoadmapSection colors={props.colors} />
    </scroll-view>
  );
}
