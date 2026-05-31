# テキストエンジンに Linebender スタックを採用する

テキスト処理に parley（text layout）・fontique（font management）・skrifa（font parsing）の Linebender スタックを採用する。cosmic-text は one-stop で使いやすいが、GPU レンダラーに Vello（同じ Linebender チーム）を採用した以上、parley + Vello の統合は同チーム設計で自然に繋がる。cosmic-text は fontdb ベースで Linebender エコシステムとは別系統であり、将来の統合コストが高くなる。Phase 0 での組み立てコストは高いが、長期の設計純粋性を優先した。
