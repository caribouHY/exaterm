# ExaTerm 開発ガイド

このガイドは、ExaTerm のローカル開発でよく使う確認手順をまとめたものです。

## セットアップ

Rust と Node.js をインストールし、リポジトリルートでフロントエンド依存関係をインストールします。

```powershell
npm install
```

## フォーマット

React、TypeScript、CSS、JSON、Markdown、YAML、Rust ファイルを整形します。

```powershell
npm run format
```

ファイルを書き換えずに整形状態だけ確認します。

```powershell
npm run format:check
```

フロントエンド側の整形は Prettier、Rust 側の整形は Cargo 経由の rustfmt で行います。

## 検証

フロントエンドをビルドします。

```powershell
npm run build
```

Rust のテストを実行します。

```powershell
cd src-tauri
cargo test
```

明示的に manifest path を指定するコマンド以外は、`src-tauri` で Cargo コマンドを実行します。

## Pull Request チェック

Pull Request では、GitHub Actions の CI workflow が `windows-latest` で実行されます。依存関係のインストール、フォーマットチェック、フロントエンドビルド、Rust テストを確認します。
