import type { HayateCssStyle } from '@tsubame/renderer-protocol';
import { inputStyle, type Palette } from '../theme';
import { type Todo } from '../todo-model';
import { editKeyAction, PRIORITY_LABEL } from '../ui/labels';
import { EASE, glow, priorityTone, titleStyle } from '../ui/styles';

function iconButton(p: Palette): HayateCssStyle {
  return {
    width: 30,
    height: 30,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: p.panel,
    defaultColor: p.muted,
    borderRadius: 8,
    borderWidth: 1,
    borderStyle: 'solid',
    borderColor: p.line,
    defaultFontSize: 14,
    ...EASE,
    ':hover': { backgroundColor: p.panel3, borderColor: p.line, defaultColor: p.text },
  };
}

export function TodoRow(props: {
  colors: Palette;
  todo: Todo;
  reorderable: boolean;
  editing: boolean;
  editDraft: string;
  onToggle: () => void;
  onRemove: () => void;
  onBeginEdit: () => void;
  onEditInput: (text: string) => void;
  onCommitEdit: () => void;
  onCancelEdit: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
}) {
  const done = props.todo.done;
  const p = props.colors;
  return (
    <view style={{
      display: 'flex',
      flexDirection: 'row',
      alignItems: 'center',
      gap: 12,
      padding: 12,
      backgroundColor: p.panel2,
      borderRadius: 12,
      borderWidth: 1,
      borderStyle: 'solid',
      borderColor: p.line,
      opacity: done ? 0.62 : 1,
      boxShadow: [{ offsetX: 0, offsetY: 2, blur: 6, spread: -1, color: p.shadow, inset: false }],
      ...EASE,
      ':hover': {
        backgroundColor: p.panel3,
        borderColor: p.line,
        boxShadow: [{ offsetX: 0, offsetY: 10, blur: 24, spread: -4, color: p.shadow, inset: false }],
      },
    }}>
      <button
        style={{
          width: 24,
          height: 24,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: done ? p.success : p.panel,
          defaultColor: p.black,
          borderRadius: 7,
          borderWidth: 1,
          borderStyle: 'solid',
          borderColor: done ? p.success : p.line,
          defaultFontSize: 14,
          boxShadow: done ? glow(`${p.success}66`) : [],
          ...EASE,
          ':hover': { borderColor: p.success },
        }}
        onClick={props.onToggle}
      >
        {done ? '✓' : ' '}
      </button>
      <view style={{
        width: 10,
        height: 10,
        backgroundColor: priorityTone(p, props.todo.prio),
        borderRadius: 999,
      }} />
      <view style={{ flexGrow: 1, display: 'flex', flexDirection: 'column' }}>
        {props.editing
          ? <text-input
            value={props.editDraft}
            style={{ ...inputStyle(p), height: 30, fontSize: 15 }}
            onInput={(event) => props.onEditInput(event.value ?? '')}
            onKeyDown={(event) => {
              const action = editKeyAction(event.key ?? '');
              if (action === 'commit') props.onCommitEdit();
              else if (action === 'cancel') props.onCancelEdit();
            }}
            onBlur={props.onCommitEdit}
          />
          : <button
            style={titleStyle(p, done)}
            onClick={props.onBeginEdit}
          >
            {props.todo.text}
          </button>}
      </view>
      {/* 優先度は左の色ドットで判る。狭幅では行が窮屈なのでラベルを畳む。 */}
      <text
        style={{ color: p.quiet, fontSize: 11 }}
        styleVariants={[{ condition: { maxWidth: 719 }, style: { display: 'none' } }]}
      >
        {`優先度 ${PRIORITY_LABEL[props.todo.prio]}`}
      </text>
      {props.reorderable
        ? <view style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 4 }}>
          <button style={iconButton(p)} onClick={props.onMoveUp}>↑</button>
          <button style={iconButton(p)} onClick={props.onMoveDown}>↓</button>
        </view>
        : null}
      <button
        style={{
          ...iconButton(p),
          ':hover': { backgroundColor: p.dangerBg, borderColor: p.danger, defaultColor: p.danger },
        }}
        onClick={props.onRemove}
      >
        ×
      </button>
    </view>
  );
}
