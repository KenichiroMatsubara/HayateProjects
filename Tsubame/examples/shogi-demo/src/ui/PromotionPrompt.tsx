import * as S from './styles.js';

/** 成る／不成の問い。盤の上に全面の受けを敷き、外側タップで取り消せる。 */
export interface PromotionPromptProps {
  readonly onChoose: (promote: boolean) => void;
  readonly onCancel: () => void;
}

export function PromotionPrompt({ onChoose, onCancel }: PromotionPromptProps) {
  return (
    <view style={S.promptScrim} onClick={onCancel}>
      <view style={S.promptCard}>
        <text style={S.promptTitle}>成りますか？</text>
        <view style={S.promptRow}>
          <button style={S.controlButton(true)} onClick={() => onChoose(true)}>
            成る
          </button>
          <button style={S.controlButton(false)} onClick={() => onChoose(false)}>
            不成
          </button>
        </view>
      </view>
    </view>
  );
}
