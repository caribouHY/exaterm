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
