import * as S from './styles.js';

/** 対局の操作。思考中は投了以外を止める（局面が動いている最中に触らせない）。 */
export interface ControlsProps {
  readonly busy: boolean;
  readonly canUndo: boolean;
  readonly isOver: boolean;
  readonly onNewGame: () => void;
  readonly onUndo: () => void;
  readonly onResign: () => void;
  readonly onFlip: () => void;
}

export function Controls({
  busy,
  canUndo,
  isOver,
  onNewGame,
  onUndo,
  onResign,
  onFlip,
}: ControlsProps) {
  return (
    <view style={S.controls}>
      <button style={S.controlButton(false)} onClick={onNewGame}>
        新規対局
      </button>
      <button
        style={S.controlButton(false)}
        onClick={!busy && canUndo ? onUndo : undefined}
      >
        待った
      </button>
      <button style={S.controlButton(false)} onClick={onFlip}>
        反転
      </button>
      <button
        style={S.controlButton(true)}
        onClick={!isOver ? onResign : undefined}
      >
        投了
      </button>
    </view>
  );
}
