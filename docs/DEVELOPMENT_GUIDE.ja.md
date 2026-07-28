# ExaTerm 開発ガイド

このガイドは、ExaTerm のローカル開発でよく使う確認手順をまとめたものです。

## セットアップ

Rust と Node.js をインストールしてから、pnpm をインストールします。

```powershell
npm install -g pnpm@10.33.2
```

リポジトリルートでフロントエンド依存関係をインストールします。

```powershell
pnpm install
```

## フォーマット

React、TypeScript、CSS、JSON、Markdown、YAML、Rust ファイルを整形します。

```powershell
pnpm run format
```

ファイルを書き換えずに整形状態だけ確認します。

```powershell
pnpm run format:check
```

フロントエンド側の整形は Prettier、Rust 側の整形は Cargo 経由の rustfmt で行います。

## 検証

フロントエンドのユニットテストを実行します。

```powershell
pnpm run test:frontend
```

フロントエンドをビルドします。

```powershell
pnpm run build
```

Rust のテストを実行します。

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

Cargo の検証コマンドは、リポジトリルートから明示的な manifest path を指定して実行します。

## ブランチとコミット

コード編集を行うときは、最新の `dev` ブランチを起点に作業ブランチを作成します。`dev` へ直接コミットせず、変更は Pull Request 経由で取り込みます。

コミットメッセージは英語で作成し、変更種別を表す prefix を付けます。

```text
feature: add SSH profile import
fix: preserve terminal size on resize
refactor: simplify config validation
docs: update development guide
test: add config parser tests
chore: update build dependencies
```

コミットはレビューしやすい粒度に分けます。1 つのコミットには原則として 1 つの目的の変更だけを含め、無関係な変更を同じコミットに混ぜないでください。

## Pull Request チェック

Pull Request は `dev` ブランチ宛てに作成します。

Pull Request では、GitHub Actions の CI workflow が `windows-latest` で実行されます。依存関係のインストール、フォーマットチェック、フロントエンドビルド、Rust テストを確認します。

## Updater の署名

Updater では専用の Tauri 署名鍵を使用します。この署名は更新がプロジェクトによって生成されたことを検証するもので、Windows Authenticode 署名とは別です。

- 秘密鍵はリポジトリ外に保管し、コミットしないでください。
- 鍵のパスワードは保護された資格情報ストアに保存してください。
- Updater 対応版の公開後は、鍵を置き換えたり紛失したりしないでください。既存のインストール環境は、別の鍵で署名された更新を拒否します。
- `src-tauri/tauri.conf.json`には公開鍵だけをコミットします。

リリースworkflowでは、次のGitHub Actions secretsを使用します。

- `TAURI_SIGNING_PRIVATE_KEY`: 秘密鍵ファイルの内容全体
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: 秘密鍵のパスワード

リリースビルドでは`src-tauri/tauri.release.conf.json`をマージし、通常のdebug buildで署名secretを要求せずに、署名済みUpdater成果物を有効にします。draft Releaseを公開する前に、MSI/NSISインストーラー、各`.sig`ファイル、`latest.json`が含まれていることを確認してください。

最初のUpdater対応版は手動でインストールする必要があります。次の正式版を使って、アプリ内更新フロー全体を確認してください。
