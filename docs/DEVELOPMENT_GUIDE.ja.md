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

フロントエンドをビルドします。

```powershell
pnpm run build
```

Rust のテストを実行します。

```powershell
cd src-tauri
cargo test
```

明示的に manifest path を指定するコマンド以外は、`src-tauri` で Cargo コマンドを実行します。

## ブランチとコミット

コード編集を行うときは、最新の `dev` ブランチを起点に作業ブランチを作成します。`dev` へ直接コミットせず、変更は Pull Request 経由で取り込みます。

作業ブランチ名には、変更内容を表す prefix を付けます。

- `feature/`: 新機能追加
- `fix/`: 不具合修正
- `refactor/`: 挙動を変えない改善
- `docs/`: ドキュメント更新
- `test/`: テスト追加・修正
- `chore/`: ビルド設定、依存関係、開発環境などの保守作業

例:

```text
feature/add-ssh-profile-import
fix/terminal-resize-glitch
refactor/config-validation
docs/update-development-guide
```

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
