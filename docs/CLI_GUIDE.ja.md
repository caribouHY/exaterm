# ExaTerm ターミナル CLI ガイド

`exaterm-cli.exe` は、ローカルプログラムや AI エージェントから、ExaTerm GUI が所有する
ターミナルセッションを MCP プロトコルなしで操作するための CLI です。MCP と同じローカル
制御プレーン、許可設定、入力検証、JSON 結果、認証入力画面を使用します。

この CLI は ExaTerm v0.7.0 以降で利用できます。

## インストールと設定

Windows インストーラーは `exaterm-cli.exe` を ExaTerm 本体と同じ場所へ配置します。
フルパスで実行するか、そのディレクトリをユーザーの `PATH` へ追加してください。

```powershell
$env:Path += ";C:\Program Files\ExaTerm"
exaterm-cli --version
```

設定画面で外部制御と CLI を有効にするか、次のように設定します。

```json
{
  "external_control": {
    "enabled": true,
    "cli_enabled": true,
    "mcp_enabled": false,
    "connect_enabled": false
  }
}
```

- `external_control.enabled` は CLI と MCP 互換アクセスに共通するマスター許可です。
- `external_control.cli_enabled` は推奨される主要経路である `exaterm-cli` を許可します。
- `external_control.connect_enabled` はプロファイル接続とシリアル接続も許可します。
- `external_control.mcp_enabled` は `exaterm-mcp` 互換アダプターだけを制御します。

設定の正本は[設定ガイド](CONFIG_JSON_GUIDE.ja.md#external_control)です。設定変更後は ExaTerm を再起動します。

## コマンド

```text
exaterm-cli sessions list
exaterm-cli profiles list [--type <ssh|telnet>]
exaterm-cli profiles connect --type <ssh|telnet> --profile-id <id> [--cols <n>] [--rows <n>]
exaterm-cli serial ports
exaterm-cli serial connect --port <name> [options]
exaterm-cli terminal output --session-id <id> --mode <recent|delta|wait> [options]
exaterm-cli terminal send --session-id <id> --data <text|->
exaterm-cli terminal run --session-id <id> --command <text|-> [options]
exaterm-cli terminal log start --session-id <id>
exaterm-cli terminal log stop --session-id <id>
```

個別の構文は `exaterm-cli <command> --help` で確認できます。

### 保存済みプロファイル

SSH と Telnet では同じ ID を使用できます。接続種別を必ず指定してください。

```powershell
exaterm-cli profiles list --type ssh
exaterm-cli profiles list --type telnet
exaterm-cli profiles connect --type ssh --profile-id router
exaterm-cli profiles connect --type telnet --profile-id router
```

`profiles list` で `--type` を省略すると、SSH と Telnet の両方を返します。
ID と種別の両方が一致する必要があります。接続には `external_control.connect_enabled=true` と、
対象プロファイルの外部制御許可が必要です。SSH パスワードや暗号化鍵のパスフレーズは
CLI 引数では受け取らず、ExaTerm UI で入力します。

### シリアル接続

`serial connect` は次の設定を受け付けます。

| オプション         | 既定値      | 許可値                                                                                                                     |
| ------------------ | ----------- | -------------------------------------------------------------------------------------------------------------------------- |
| `--baud-rate`      | `9600`      | 1 以上の整数                                                                                                               |
| `--data-bits`      | `8`         | `5`, `6`, `7`, `8`                                                                                                         |
| `--parity`         | `none`      | `none`, `odd`, `even`                                                                                                      |
| `--stop-bits`      | `1`         | `1`, `2`                                                                                                                   |
| `--flow-control`   | `none`      | `none`, `software`, `hardware`                                                                                             |
| `--terminal-mode`  | `general`   | `general`, `cisco-ios`, `arista-eos`, `juniper-junos`, `vyos`, `fujitsu-sir`, `allied-telesis-awplus`, `furukawa-fitelnet` |
| `--cols`, `--rows` | `120`, `30` | `1` から `1000`                                                                                                            |

ポート名は `serial ports` が返す値と完全一致する必要があります。

## 出力の読み取り

出力文字数の既定値は 2,000、上限は 20,000 です。

保持されている直近出力を取得します。

```powershell
exaterm-cli terminal output --session-id $session --mode recent --max-chars 2000
```

返されたカーソル以降を取得します。

```powershell
exaterm-cli terminal output --session-id $session --mode delta --cursor 1200
```

新しい出力または指定文字列を待機します。

```powershell
exaterm-cli terminal output --session-id $session --mode wait `
  --cursor 1200 --contains "router#" --timeout-ms 30000
```

`delta` では `--cursor` が必須です。`wait` で省略すると現在位置から待機します。
待機時間の既定値は 10 秒、上限は 60 秒です。

## 入力送信とコマンド実行

値に `-` を指定すると stdin から読み取ります。シェルのクォート問題を避け、複数行入力を
渡せます。

```powershell
"show version`n" | exaterm-cli terminal send --session-id $session --data -
```

```powershell
@"
show interfaces
show ip route
"@ | exaterm-cli terminal run --session-id $session --command - --wait-contains "router#"
```

`terminal run` は既定で改行を追加します。無効化するには `--append-newline false` を指定します。
`--timeout-ms`、`--settle-ms`（上限 5,000）、`--max-chars` も使用できます。

## JSON 出力と終了コード

成功時は対応する MCP ツールと同じ JSON を stdout へ出力します。エラーは stderr へ
JSON で出力します。

```json
{ "error": { "code": "cli_disabled", "message": "..." } }
```

| 終了コード | 意味                                             |
| ---------- | ------------------------------------------------ |
| `0`        | 成功                                             |
| `1`        | 設定、GUI 起動、制御プレーン、ツール実行のエラー |
| `2`        | CLI 引数または stdin 入力のエラー                |

`--help` と `--version` だけは人向けテキストです。

## GUI と認証情報

ExaTerm が停止中の場合、CLI は通常の表示される GUI を起動し、ローカル制御プレーンを
最大 30 秒待機します。セッションはGUIが所有し続けます。プロファイル接続は通常のタブとして
表示され、必要な SSH 認証情報は GUI で入力します。

## セキュリティ

ターミナル出力、コマンド、プロンプト、プロファイルメモ、ホスト名、ユーザー名、ログパスには
機密情報が含まれる可能性があります。信頼済みローカルプログラムに対してのみ CLI を有効に
してください。保存済み認証情報、API キー、秘密鍵本文、ログ本文は CLI から公開しません。
セッションログは平文であり、接続時のログ開始設定が有効な場合、または明示的にログを開始した場合だけ作成されます。

## トラブルシューティング

- `cli_disabled`: `external_control.enabled` と `external_control.cli_enabled` を有効にして再起動します。
- プロファイル/シリアル接続が拒否される: `external_control.connect_enabled` を有効にします。
- セッションが見つからない: `sessions list` の `session_id` を使用します。
- 待機がタイムアウトする: `timed_out` と出力を確認し、返された `cursor` から継続します。
- GUI を利用できない: `exaterm.exe` が CLI と同じインストール先にあり、起動できるか確認します。

## AI エージェントからの利用例

```powershell
$sessions = exaterm-cli sessions list | ConvertFrom-Json
$session = $sessions.sessions[0].session_id
$result = exaterm-cli terminal run --session-id $session `
  --command "show version" --wait-contains "#" --timeout-ms 30000 | ConvertFrom-Json
$result.output
```

破壊的なコマンドをエージェントが選択する場合は、アプリケーション側の承認ポリシーを必須に
してください。
