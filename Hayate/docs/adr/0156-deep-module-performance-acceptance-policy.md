---
status: accepted
---

# Deep module性能変更のacceptance policy

性能改善では操作中のFrame Deadline逸脱を減らすことをprimary goalとし、実機のrepresentative workloadと候補別のparameterized stress workloadの二層で評価する。representative workloadは同一端末・同一build・同一操作を最低5回実行して中央値と分布を比較し、単発値を採否根拠にしない。stress workloadはlayer数・dirty比率、SceneGraph規模と深さ、sibling数とz-index変更率、resource pressure、wake頻度とrefresh rateを候補に応じて変化させる。

採否に使う実機buildはproductionと同じrelease optimization、renderer選択、assets、font、surface設定を持つ専用のprofileable benchmark buildとする。debug assertionsと`scene-validation`は無効にし、performance observabilityだけをcompile-timeで有効化する。観測はfixed-size counterとPerfetto markerを使い、大量のper-frame log、dev reload、Hermes debug instrumentation、不要なdiagnostic pollingを含めない。Androidはdebuggerなしでprofile可能にする。

cold startは各runでprocessを終了してから測り、steady-stateはwarmup完了後に測る。thermal status、refresh rate、battery power stateをrunごとに記録する。debug buildのlogと計測は原因調査に使えるが採否値にせず、full structural validationと契約検証は別のvalidation buildで行う。production buildへ観測を常時載せることも、benchmark buildでvalidation costを混ぜることも採らない。

接続実機が対応する60Hzと90Hzをadaptive refreshなしで固定して測る。60Hzは16.67msのrepresentative gate、90Hzは11.11msのhigh-refresh stress gateとし、それぞれ最低5 run実施する。45Hzは性能stressにならないため通常matrixから除外し、120Hz実機が加わった時点で8.33ms gateを追加する。host stress fixtureは先に120Hz budgetも検証する。ADB等でrefresh rateを一時変更した場合は試験終了時に元の設定へ戻す。

性能gateは中間commitではなく、完成したdeep moduleまたはworkstream単位で適用する。Scene Snapshot / index、Committed Frame / Layer Topology、Layer Scene / Layer Presentationは一つのScene Pipeline clusterとして評価し、途中で一時的なcostが増えても個別却下しない。観測経路、retained Paint Order、Android Latest-Wins Frame Scheduling、Render Resource Residencyは独立workstreamとする。各中間commitはcorrectness、parity、invariantを満たし、完成時に性能を判定する。

deep module化によってlocality、leverage、testabilityが明確に改善する場合、representative workloadで性能向上が測定できない変更や小幅なconstant cost増加を許容する。ただし次のmaterial regressionが一つでもある変更は、構造上の利点にかかわらず採用しない。

- correctness、pixel parity、入力結果の退行
- 2 refresh intervalを超えるframe発生数の統計的増加
- 対象frame phaseのp95が10%以上かつ0.25ms以上悪化
- steady-state CPU/GPU resident memoryが10%以上かつ8MB以上増加
- startup p95が10%以上かつ25ms以上悪化
- idle時の継続的なCPU、GPU、timer、vsync callbackの復活
- thermal statusまたはOS memory-pressure挙動の一段階悪化
- stress workloadにおける計算量またはmemory量のscaling class悪化

割合と絶対値の両方を超えた場合だけmaterial regressionとし、小さい処理の測定noiseを退行扱いしない。material regression未満なら、性能同等、小さなCPU/memory trade-off、stress改善未観測であっても、deep moduleとしての構造改善を採用できる。改善不足時に旧経路をruntime fallbackとして残すことはせず、moduleをさらに深めるか変更全体を戻す。

## Considered Options

- measurable performance improvementを全変更の必須条件にすると、複数段階で完成するdeep moduleの中間costや、将来の変更局所性を得る構造改善を拒むため不採用。
- architecture改善だけで無制限の性能悪化を許容する案はprimary goalと矛盾するため不採用。
- 単一runまたは割合だけのthresholdは端末noiseと小さい絶対差を過大評価するため不採用。

## Consequences

- performance reportはrepresentative/stress、最低5 run、割合と絶対差を併記する。
- performance reportはbenchmark buildのcommit、端末、refresh rate、thermal、power、warm/cold条件を記録する。
- reviewerは性能向上の有無とmaterial regressionの有無を分けて判断する。
- module内部のstorage方式などinterfaceへ漏れない選択は、同じpolicy内でbenchmarkにより選べる。
