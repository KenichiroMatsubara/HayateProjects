import type { HayateCssStyle } from '@tsubame/renderer-protocol';
import { type Palette } from '../theme';
import { type Filter, type SortMode } from '../todo-model';
import { FILTERS, SORTS } from '../ui/labels';
import { EASE, glow } from '../ui/styles';

function chipStyle(p: Palette, active: boolean): HayateCssStyle {
  return {
    height: 30,
    paddingLeft: 12,
    paddingRight: 12,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: active ? p.accent : p.panel2,
    defaultColor: active ? p.black : p.text,
    borderRadius: 999,
    borderWidth: 1,
    borderStyle: 'solid',
    borderColor: active ? p.accent : p.line,
    defaultFontSize: 12,
    boxShadow: active ? glow(`${p.accent}44`) : [],
    ...EASE,
    ':hover': {
      backgroundColor: active ? p.accent : p.panel3,
      borderColor: active ? p.accent : p.line,
    },
  };
}

export function Toolbar(props: {
  colors: Palette;
  filter: Filter;
  sort: SortMode;
  onFilter: (filter: Filter) => void;
  onSort: (sort: SortMode) => void;
}) {
  return (
    <view style={{
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      flexWrap: 'wrap',
      gap: 8,
      paddingTop: 10,
      paddingBottom: 10,
    }}>
      <text style={{ color: props.colors.quiet, fontSize: 12 }}>表示</text>
      {FILTERS.map((item) => (
        <button style={chipStyle(props.colors, props.filter === item.value)} onClick={() => props.onFilter(item.value)}>
          {item.label}
        </button>
      ))}
      <view style={{ width: 1, height: 18, marginLeft: 4, marginRight: 4, backgroundColor: props.colors.line }} />
      <text style={{ color: props.colors.quiet, fontSize: 12 }}>並び</text>
      {SORTS.map((item) => (
        <button style={chipStyle(props.colors, props.sort === item.value)} onClick={() => props.onSort(item.value)}>
          {item.label}
        </button>
      ))}
    </view>
  );
}
