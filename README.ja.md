<p align="center">
  <img src="src-tauri/icons/128x128.png" width="128" height="128" alt="ExaTerm icon">
</p>

<h1 align="center">ExaTerm</h1>

<p align="center">
  <a href="https://github.com/caribouHY/exaterm/releases"><img src="https://img.shields.io/github/v/release/caribouHY/exaterm?display_name=tag&label=release" alt="Latest release"></a>
  <a href="#対応os"><img src="https://img.shields.io/badge/platform-Windows-0078D6?logo=windows&logoColor=white" alt="Windows"></a>
  <a href="https://app.codacy.com/gh/caribouHY/exaterm/dashboard?utm_source=gh&utm_medium=referral&utm_content=&utm_campaign=Badge_grade"><img src="https://app.codacy.com/project/badge/Grade/ea195c48da78487a872c769c4e5815f5" alt="Codacy Badge"></a>
  <a href="https://github.com/caribouHY/exaterm/releases"><img src="https://img.shields.io/github/downloads/caribouHY/exaterm/total?label=downloads" alt="Downloads"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/caribouHY/exaterm?label=license" alt="License"></a>
</p>

ExaTermはAIエージェント連携とAIチャットに対応した、SSH/Telnet/シリアル通信用のターミナルアプリです。

![window image](docs/images/window.png)

## 機能

- SSH、Telnet、シリアル通信
- AIエージェント向けCLIツールとSkill
- ターミナル内蔵AIチャット
- ログ機能（手動/自動選択可能）
- ネットワーク機器向け色分け機能（Cisco IOS・Arista EOS・VyOS・Fujitsu Si-R・Allied Telesis AW+・Furukawa FITELnet対応）

## 対応OS

ExaTermは現在Windowsのみ対応しています。

現時点ではmacOSやLinuxはサポートしていません。

## インストール

wingetコマンドでインストール可能です。

```powershell
winget install caribouhy.ExaTerm
```

もしくは本リポジトリの[Releasesページ](https://github.com/caribouHY/exaterm/releases)からインストーラーをダウンロードしてください。

## コマンドラインからの起動

`exaterm.exe`に引数を渡すと、ExaTermを起動してSSHまたはTelnet接続を開始できます。接続先にはホスト名、IPアドレス、または保存済みプロファイル名を指定できます。

```powershell
exaterm.exe ssh <user@hostname-or-ip-address|profile-name>
exaterm.exe telnet <hostname-or-ip-address|profile-name>
exaterm.exe help
```

## AIエージェント連携

ExaTermに同梱されているCLIツール`exaterm-cli`と専用のAgent Skillを組み合わせることで、ClaudeやCodexなどのAIエージェントからExaTermを操作できます。
`exaterm-cli`を利用するには、設定画面で「外部制御を有効化」と「ターミナル CLI を有効化」を有効にする必要があります。ツールの利用方法は[ターミナル CLI ガイド](docs/CLI_GUIDE.ja.md)を参照してください。
外部制御から新しいSSH/Telnet接続を開始する場合は、「外部からの新規接続を許可」を有効にし、外部制御を許可した接続先プロファイルを事前に作成してください。

### Agent Skill

対応するAIエージェントから利用するには、このリポジトリの `exaterm-cli` Skillをインストールします。

```powershell
npx skills add caribouHY/exaterm --skill exaterm-cli
```

対象のエージェントを限定する場合は、`-a`オプションを指定して実行します。

```powershell
npx skills add caribouHY/exaterm --skill exaterm-cli -a codex
npx skills add caribouHY/exaterm --skill exaterm-cli -a claude-code
npx skills add caribouHY/exaterm --skill exaterm-cli -a github-copilot
```

SkillにExaTerm本体は含まれません。ExaTermを別途インストールし、設定画面で「外部制御を有効化」と「ターミナル CLI を有効化」を有効にしてください。

### MCP連携

`exaterm-mcp.exe`によるstdio MCP互換アダプターに対応しています。利用するには、設定画面で「外部制御を有効化」と「MCP 互換アダプターを有効化」を有効にしてください。

外部制御から新しいSSH/Telnet接続を開始する場合は、「外部からの新規接続を許可」を有効にし、外部制御を許可した接続先プロファイルを事前に作成する必要があります。

外部制御ではターミナル出力の読み取りや、入力およびコマンドの送信が可能です。ターミナル内容には機密情報が含まれる可能性があるため、信頼できるローカルクライアントに対してのみ有効にしてください。

## AIアシスタント

AIアシスタントは、アクティブなタブのターミナル内容を参照して、出力の説明やコマンド候補の提示を支援できます。提示されたコマンドは、ターミナルで実行する前に必ず内容を確認してください。

OpenAI、Azure OpenAI、Anthropic、Gemini、OpenRouterを使用するには、各プロバイダーのAPIキーまたはエンドポイント設定が必要です。認証情報の保存や削除は設定から行います。

Ollamaは通常APIキーを必要としませんが、ExaTermから到達できるOllamaサーバーが起動している必要があります。

## 設定ファイル等の保存先

設定ファイルやログ等は下記の場所に保存されます。

| データ               | 保存場所                        |
| -------------------- | ------------------------------- |
| 設定                 | `%AppData%\ExaTerm\config.json` |
| 任意のセッションログ | `%AppData%\ExaTerm\logs`        |
| SSH known hosts      | `%AppData%\ExaTerm\known_hosts` |
| AIサービスのAPIキー  | OSの資格情報ストア              |

- [設定ガイド](docs/CONFIG_JSON_GUIDE.ja.md)
- [ターミナル CLI ガイド](docs/CLI_GUIDE.ja.md)
- [ドキュメント索引](docs/README.ja.md)

## 開発者向けセットアップ

開発者向けセットアップにはRustとNode.jsが必要です。

1. リポジトリをクローンします。
2. pnpmをインストールします。

```powershell
npm install -g pnpm@10.33.2
```

3. 依存関係をインストールします。

```powershell
pnpm install
```

4. Tauri開発版アプリを起動します。

```powershell
pnpm run tauri dev
```

5. フロントエンドをビルドします。

```powershell
pnpm run build
```

6. Windows向けexeパッケージをビルドします。

```powershell
pnpm run tauri build
```

## ライセンス

ExaTermはMIT Licenseの下で公開されています。詳細は[LICENSE](LICENSE)を参照してください。
