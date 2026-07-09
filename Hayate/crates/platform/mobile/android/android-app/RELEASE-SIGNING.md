# Android リリース署名と Google Play 内部テスト

鳥見（Torimi）ホスト APK/AAB を Google Play の**内部テスト**トラックへ出すための手順。

`app/build.gradle.kts` は release 署名を**環境変数か Gradle プロパティから読む**よう構成済み
（keystore とパスワードはリポジトリに置かない）。4 つの値がすべて揃ったときだけ release 署名を
構成し、未設定なら従来どおり未署名で、`assembleDebug` などデバッグビルドは一切影響を受けない。

このドキュメントが扱うのは、コードにできない**人力スライス**（keystore の生成と保管、Play Console
の操作）と、その値の渡し方だ。

---

## 前提（適用範囲）

- **ABI は arm64-v8a のみ**（wgpu が Android では Vulkan 前提・`build.gradle.kts`）。x86/armeabi 端末・
  多くのエミュレータでは動かない。内部テスターの実機が arm64（現行 Android 端末はほぼこれ）である
  こと。
- 現在の `applicationId`（Play 公開パッケージ名・永久固定）は `com.hayateprojects.torimi`
  （code パッケージ／namespace は `com.hayateprojects.hayate.adapter_android_demo` のまま）、`versionName=0.1.0` /
  `versionCode=1`。**`applicationId` は Play 公開後は永久固定**なので、内部テストとはいえ最初の
  アップロード前に「これを正式パッケージ名にしてよいか」を確定させること（変える場合は
  `build.gradle.kts` の `namespace` と `applicationId` を先に直す）。
- Play へ 2 回目以降を上げるたびに `versionCode` を +1 する必要がある（同一 versionCode は拒否される）。

---

## 1. アップロード用 keystore を生成する（人力・一度きり）

リポジトリの**外**（例: `~/keystores/`）に置く。生成物とパスワードは厳重に保管し、**紛失すると
アップロード鍵をローテーションするまで更新を出せなくなる**（Play App Signing 利用時は復旧可能だが手間）。

```sh
keytool -genkeypair -v \
  -keystore ~/keystores/torimi-upload.jks \
  -alias torimi-upload \
  -keyalg RSA -keysize 2048 -validity 10000 \
  -storetype JKS
```

対話で店舗パスワード・鍵パスワード・氏名/組織などを入力する。ここで決めた
**keystore パス・store パスワード・alias・key パスワード**を次段で渡す。

> Play App Signing（推奨・既定）を使う場合、ここで作るのは**アップロード鍵**。Play が配布用の
> **アプリ署名鍵**を別に管理するので、アップロード鍵を万一失っても Play Console から再登録できる。

---

## 2. 署名の値を Gradle に渡す

`build.gradle.kts` は以下の Gradle プロパティ（優先）または環境変数から読む:

| Gradle プロパティ | 環境変数 | 意味 |
| --- | --- | --- |
| `hayate.release.storeFile` | `HAYATE_RELEASE_STORE_FILE` | keystore の絶対パス |
| `hayate.release.storePassword` | `HAYATE_RELEASE_STORE_PASSWORD` | store パスワード |
| `hayate.release.keyAlias` | `HAYATE_RELEASE_KEY_ALIAS` | 鍵 alias（例 `torimi-upload`） |
| `hayate.release.keyPassword` | `HAYATE_RELEASE_KEY_PASSWORD` | 鍵パスワード |

### ローカル（`~/.gradle/gradle.properties` に書く — リポジトリ管理外）

```properties
hayate.release.storeFile=/home/you/keystores/torimi-upload.jks
hayate.release.storePassword=********
hayate.release.keyAlias=torimi-upload
hayate.release.keyPassword=********
```

### CI（GitHub Actions Secrets 経由・環境変数で渡す）

keystore そのものは Secret に base64 で入れて実行時に復元し、パスワード類は Secret を環境変数へ。
（AAB を CI でビルドする場合。初回は手動アップロードで足りるので CI 化は任意 — 下記 4 を参照。）

---

## 3. 署名済み AAB をビルドする

```sh
cd Hayate
./scripts/build-android.sh bundleRelease
```

成果物: `crates/platform/mobile/android/android-app/app/build/outputs/bundle/release/app-release.aab`

- ビルドは Rust の cargo（`libhayate_adapter_android.so`）を含むため、Android SDK/NDK と Rust
  ツールチェインが要る。`build-android.sh` が JDK/SDK を解決する（推奨）。
- 署名の 4 値が未設定だと**未署名 AAB**ができる（Play には出せない）。ビルドログの
  `signingConfig` が付いているか、または `bundletool`/`jarsigner -verify` で確認するとよい。

デバッグ実機確認だけなら従来どおり:

```sh
pnpm --filter hayate run torimi:android:install   # = build-android.sh installDebug
```

---

## 4. Google Play Console 側（人力）

**初回はここが必須で、CI 自動アップロードだけでは完結しない**（アプリ本体を Console 上で作る必要が
あり、API アップロードは既存アプリにしか効かない）。

1. **アプリを作成** — Play Console → アプリを作成 → 名前・言語・アプリ/ゲーム・無料/有料を設定。
   `applicationId`（= 上記）がこのアプリに紐づく。
2. **Play App Signing を有効化**（既定で有効）。アップロード鍵は手順 1 の keystore。
3. **内部テストトラック** — テスト → 内部テスト → 新しいリリースを作成。
4. **AAB をアップロード** — 手順 3 で作った `app-release.aab` をドラッグ&ドロップ。
5. **テスターを登録** — 内部テスト用のメールリスト（最大 100 名）を作り、オプトイン URL を共有。
6. 審査（内部テストは軽い）を経て、テスターがリンクからインストール可能になる。

初回リリース時に Play が求める項目（プライバシーポリシー URL、データセーフティ、対象年齢、
コンテンツレーティング等）も内部テスト公開前に埋める必要がある。

---

## 5. （任意）CI で AAB をビルド／アップロードする

初回手動アップロードでアプリが Play に存在するようになったら、2 回目以降は自動化できる。
`.github/workflows/torimi-release.yml` は「将来の AAB ビルドは同じ `torimi-android-v*` タグに
乗る別ジョブとして並ぶ」設計（ADR-0003）。その際に必要な追加 Secrets の目安:

| Secret | 用途 |
| --- | --- |
| `ANDROID_KEYSTORE_BASE64` | keystore を base64 化したもの（実行時に復元） |
| `HAYATE_RELEASE_STORE_PASSWORD` / `_KEY_ALIAS` / `_KEY_PASSWORD` | 手順 2 の値 |
| `PLAY_SERVICE_ACCOUNT_JSON` | Play Developer API のサービスアカウント鍵（内部テストへの自動アップロード用） |

サービスアカウントは Google Cloud で作成し、Play Console の API アクセスで対象アプリに権限を付与する。
アップロードは `r0adkll/upload-google-play` などの Action か `bundletool`+Play Developer API で行う。
