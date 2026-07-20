import type { HayateCssStyle } from '@torimi/tsubame-renderer-protocol';
import { inputStyle, type Palette } from '../theme';
import { type Priority } from '../todo-model';
import { PRIORITIES, PRIORITY_LABEL } from '../ui/labels';
import { EASE, glow, priorityTone } from '../ui/styles';

export function AddForm(props: {
  colors: Palette;
  draft: string;
  prio: Priority;
  onInput: (text: string) => void;
  onPrio: (prio: Priority) => void;
  onAdd: () => void;
}) {
  const seg = (active: boolean, tone: string): HayateCssStyle => ({
    height: 38,
    minWidth: 40,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: active ? tone : props.colors.panel2,
    defaultColor: active ? props.colors.black : props.colors.muted,
    borderRadius: 9,
    borderWidth: 1,
    borderStyle: 'solid',
    borderColor: active ? tone : props.colors.line,
    defaultFontSize: 13,
    boxShadow: active ? glow(`${tone}55`) : [],
    ...EASE,
    ':hover': {
      backgroundColor: active ? tone : props.colors.panel3,
      borderColor: active ? tone : props.colors.line,
    },
  });

  return (
    <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', flexWrap: 'wrap', gap: 8 }}>
      <view style={{ flexGrow: 1, minWidth: 180 }}>
        <text-input
          value={props.draft}
          placeholder="新しいタスクを入力…"
          style={inputStyle(props.colors)}
          onInput={(event) => props.onInput(event.value ?? '')}
          onKeyDown={(event) => {
            if (event.key === 'Enter') props.onAdd();
          }}
        />
      </view>
      <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 4 }}>
        {PRIORITIES.map((prio) => (
          <button
            style={seg(props.prio === prio, priorityTone(props.colors, prio))}
            onClick={() => props.onPrio(prio)}
          >
            {PRIORITY_LABEL[prio]}
          </button>
        ))}
      </view>
      <button
        style={{
          height: 38,
          paddingLeft: 18,
          paddingRight: 18,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: props.colors.accent,
          defaultColor: props.colors.black,
          borderRadius: 9,
          borderWidth: 1,
          borderStyle: 'solid',
          borderColor: props.colors.accent,
          defaultFontSize: 13,
          boxShadow: glow(`${props.colors.accent}55`),
          ...EASE,
          ':hover': {
            backgroundColor: props.colors.success,
            borderColor: props.colors.success,
            boxShadow: glow(`${props.colors.success}77`, true),
          },
        }}
        onClick={props.onAdd}
      >
        追加
      </button>
    </view>
  );
}
