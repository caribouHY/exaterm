# ExaTerm config.json パラメータ説明

この資料は、ExaTerm のユーザー設定ファイル `config.json` を手動で確認・編集するための説明です。

## ファイルの場所

Windows では、設定ファイルは通常次の場所に保存されます。

```text
%AppData%\ExaTerm\config.json
```

実際のパス例:

```text
C:\Users\<ユーザー名>\AppData\Roaming\ExaTerm\config.json
```

`config.json` が存在しない場合、ExaTerm の起動時または設定読み込み時に既定値で自動作成されます。

## 編集前の注意

- 編集前に `config.json` のバックアップを作成してください。
- ExaTerm を終了してから編集することを推奨します。起動中に設定画面で保存すると、手動編集した内容が上書きされる場合があります。
- JSON 形式のため、末尾の余分なカンマや引用符の不足があると読み込みに失敗します。
- API キーは `config.json` ではなく、OS の資格情報ストアに保存されます。

## 設定例

```json
{
  "config_version": 1,
  "language": "ja",
  "ai": {
    "azure_openai_enabled": false,
    "azure_openai_endpoint": "",
    "azure_openai_deployment": "",
    "ollama_enabled": false,
    "ollama_base_url": "http://localhost:11434",
    "default_provider": "OpenAi",
    "default_model": "gpt-4o",
    "debug_log_enabled": false
  },
  "mcp": {
    "enabled": false,
    "connect_enabled": false,
    "stdio_enabled": false
  },
  "terminal": {
    "font_size": 14,
    "font_family": "Consolas, 'Courier New', monospace",
    "cursor_style": "block",
    "scrollback": 10000,
    "auto_session_log": false,
    "log_format": "display",
    "include_log_header": true
  },
  "ssh": {
    "allow_legacy_algorithms": false
  },
  "saved_connections": []
}
```

## ルート項目

| パラメータ          | 型     | 既定値   | 説明                                                                                                                 |
| ------------------- | ------ | -------- | -------------------------------------------------------------------------------------------------------------------- |
| `config_version`    | number | `1`      | 設定ファイルのバージョンです。通常は変更しません。古い設定を読み込んだ場合、ExaTerm が現在のバージョンへ更新します。 |
| `language`          | string | `"en"`   | 画面表示言語です。`"en"` は英語、`"ja"` は日本語です。                                                               |
| `ai`                | object | 下記参照 | AI アシスタント関連の設定です。                                                                                      |
| `mcp`               | object | 下記参照 | 外部 AI エージェントからターミナルを制御するためのローカル MCP 設定です。                                            |
| `terminal`          | object | 下記参照 | ターミナル表示とログ関連の設定です。                                                                                 |
| `ssh`               | object | 下記参照 | SSH 接続の互換性設定です。                                                                                           |
| `saved_connections` | array  | `[]`     | 保存済み SSH/Telnet 接続プロファイルです。プロファイルは接続ダイアログから作成、選択、削除できます。                 |

## ai

| パラメータ                   | 型      | 既定値                     | 説明                                                                                                                                                                                                                               |
| ---------------------------- | ------- | -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ai.azure_openai_enabled`    | boolean | `false`                    | `true` にすると、Endpoint、モデルデプロイ名、API キーが設定されている場合に Azure OpenAI を AI パネルに表示します。                                                                                                                |
| `ai.azure_openai_endpoint`   | string  | `""`                       | リクエスト送信先の Azure OpenAI chat completions URL 全体です。例: `"https://your-resource.openai.azure.com/openai/v1/chat/completions"`、または `api-version` 付きのデプロイ URL。ExaTerm は入力された URL をそのまま使用します。 |
| `ai.azure_openai_deployment` | string  | `""`                       | Azure OpenAI のモデルデプロイ名です。v1 API では、この値を `model` フィールドとして送信します。                                                                                                                                    |
| `ai.ollama_enabled`          | boolean | `false`                    | `true` にすると、Ollama のモデルを AI パネルに表示します。Ollama を使用するには、ローカルまたは指定 URL の Ollama サーバーが起動している必要があります。                                                                           |
| `ai.ollama_base_url`         | string  | `"http://localhost:11434"` | Ollama API のベース URL です。ローカル環境の標準設定では `"http://localhost:11434"` を使用します。空文字の場合、画面上では既定 URL として扱われます。                                                                              |
| `ai.default_provider`        | string  | `"OpenAi"`                 | AI パネルで優先的に選択されるプロバイダです。使用可能な値は `"OpenAi"`, `"AzureOpenAi"`, `"Anthropic"`, `"Gemini"`, `"OpenRouter"`, `"Ollama"` です。                                                                              |
| `ai.default_model`           | string  | `"gpt-4o"`                 | AI パネルで優先的に選択されるモデル ID です。設定画面からは現在直接編集できないため、必要な場合は手動で編集します。保存したモデルが利用できない場合は、利用可能なモデルへ自動的にフォールバックします。                            |
| `ai.debug_log_enabled`       | boolean | `false`                    | `true` にすると、AI チャットのリクエストと応答を JSON Lines 形式のデバッグログとして `%AppData%\ExaTerm\ai-debug` に保存します。                                                                                                   |

### AI API キーについて

OpenAI、Azure OpenAI、Anthropic、Google Gemini、OpenRouter の API キーは `config.json` には保存されません。設定画面で登録したキーは、OS の資格情報ストアに保存されます。

Azure OpenAI の chat completions URL 全体とモデルデプロイ名を設定してください。ExaTerm は Endpoint URL を入力どおりに使用し、パスや `api-version` は自動付与しません。

Ollama は通常 API キーを必要としません。`ai.ollama_enabled` と `ai.ollama_base_url` を設定してください。

### AI デバッグログ

AI チャットの動作を調査する必要がある場合のみ、`ai.debug_log_enabled` を `true` にしてください。デバッグログは `%AppData%\ExaTerm\ai-debug\YYYYMMDD.log` に保存され、プロンプト、AI 応答、ターミナルコンテキストの本文を含む可能性があります。API キーと HTTP ヘッダーは保存されません。

### 代表的なモデル ID

| プロバイダ   | モデル ID の例                                          |
| ------------ | ------------------------------------------------------- |
| OpenAI       | `gpt-4o`, `gpt-4o-mini`                                 |
| Azure OpenAI | Azure のモデルデプロイ名。例: `my-gpt4o`                |
| Anthropic    | `claude-sonnet-4-20250514`, `claude-3-5-haiku-20241022` |
| Gemini       | `gemini-2.5-pro`, `gemini-2.5-flash`                    |
| OpenRouter   | `openai/gpt-4o`, `anthropic/claude-sonnet-4`            |
| Ollama       | ローカルの Ollama にインストール済みのモデル名          |

## mcp

| パラメータ            | 型      | 既定値  | 説明                                                                                                                                                                                                                                                                                                 |
| --------------------- | ------- | ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mcp.enabled`         | boolean | `false` | ローカル MCP アクセスのマスター許可です。外部 MCP クライアントを使用するには、対応トランスポートの設定とあわせて有効化します。                                                                                                                                                                       |
| `mcp.connect_enabled` | boolean | `false` | `true` にすると、信頼済み MCP クライアントが保存済み SSH/Telnet プロファイルの一覧取得、そのプロファイルからの新規タブ作成、シリアルポート一覧取得、シリアルコンソール接続を行えます。SSH パスワードや暗号化鍵のパスフレーズは ExaTerm UI で入力し、MCP クライアントへは公開されず保存もされません。 |
| `mcp.stdio_enabled`   | boolean | `false` | `mcp.enabled=true` と併用して `true` にすると、ローカル MCP クライアントから `exaterm-mcp` stdio proxy を使用できます。proxy は current-user ローカル control plane 経由で、実際のツール呼び出しを起動中の ExaTerm GUI へ転送します。                                                                |

HTTP MCP transport は削除されました。古い設定ファイルに残っている HTTP 専用の host と port の値は無視され、次に ExaTerm が設定を保存したときに出力されなくなります。以前 HTTP MCP を使用していた場合は、`mcp.enabled=true` と `mcp.stdio_enabled=true` を設定し、MCP クライアント側で `exaterm-mcp` を起動するよう手動で設定してください。

### MCP ツール

MCP が有効な場合、外部クライアントは次のツールを呼び出せます。

- `list_terminal_sessions`: ユーザーが ExaTerm で開いたターミナルセッションを一覧表示します。
- `read_terminal_output`: セッションの直近出力を読み取ります。返却値には次回差分読み取りに使える `cursor` が含まれます。
- `read_terminal_output_delta`: 指定した `cursor` 以降の出力だけを読み取ります。
- `wait_terminal_output`: 新しい出力、または指定した文字列が出力に現れるまで待機します。
- `send_terminal_input`: 接続中のセッションへテキストを送信します。
- `run_terminal_command`: 接続中のセッションへコマンドを送信し、出力待機後に差分出力を返します。
- `start_terminal_log`: 接続中セッションの手動平文ログを開始します。ログは `%AppData%\ExaTerm\logs` 配下に保存され、返却値には作成されたファイルパスが含まれます。
- `stop_terminal_log`: ExaTerm が表示済み出力をログへ flush した後、セッションの手動平文ログを停止します。

`mcp.connect_enabled` も `true` の場合、外部クライアントは次の追加ツールを呼び出せます。

- `list_connection_profiles`: 保存済み SSH/Telnet プロファイルを一覧表示します。秘密鍵パスや認証情報は返しません。
- `connect_saved_profile`: 保存済みプロファイルから新しい SSH/Telnet セッションを開き、ExaTerm のタブとして表示します。SSH パスワードや暗号化鍵のパスフレーズは MCP ツール引数では受け取らず、ExaTerm UI で入力します。
- `list_serial_ports`: 現在利用可能なシリアルポートを一覧表示します。
- `connect_serial_console`: MCP ツール引数で指定したポート名と通信設定から新しいシリアルコンソールセッションを開き、ExaTerm のタブとして表示します。ポート名は利用可能なシリアルポートと完全一致する必要があります。

出力読み取り系ツールは `start_cursor`、`cursor`、`truncated` を返します。`cursor` は次回の `read_terminal_output_delta` や `wait_terminal_output` に渡せます。古い出力が内部バッファから切り詰められている場合、`truncated` は `true` になります。

`wait_terminal_output` と `run_terminal_command` の待機時間は最大 60 秒です。`run_terminal_command` は既存の接続済みセッションだけを対象とします。保存済みプロファイルからの新規接続には、明示的に有効化した `connect_saved_profile` を使用します。

MCP サーバーは保存済み認証情報の読み取り、API キーの公開、ログファイルの直接読み取りを行いません。MCP クライアントはセッションログを明示的に開始・停止できますが、受け取るのはログ状態とファイルパスだけで、ログ本文ではありません。MCP 経由の SSH/Telnet 新規接続は保存済みプロファイルに限定され、シリアル接続は明示的に指定した利用可能ポート名だけを対象にし、すべての MCP 新規接続には `mcp.connect_enabled=true` が必要です。SSH では既存の known_hosts 検証もそのまま適用されます。SSH プロファイルに `jump_profile_id` がある場合、MCP 経由の新規接続でも同じ 1 段の踏み台フローを使用し、必要な踏み台認証情報は ExaTerm UI で入力します。ターミナル出力やログファイルには機密情報が含まれる可能性があるため、MCP と MCP 経由のログ開始は信頼できるローカルクライアントに対してのみ有効化してください。

## terminal

| パラメータ                    | 型      | 既定値                                 | 説明                                                                                                                                                            |
| ----------------------------- | ------- | -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `terminal.font_size`          | number  | `14`                                   | ターミナルのフォントサイズです。設定画面では `8` から `32` の範囲で指定できます。                                                                               |
| `terminal.font_family`        | string  | `"Consolas, 'Courier New', monospace"` | ターミナルのフォントファミリーです。CSS の `font-family` と同じ形式で指定します。                                                                               |
| `terminal.cursor_style`       | string  | `"block"`                              | ターミナルのカーソル形状です。現在の既定値はブロックカーソルです。手動編集時は xterm.js が受け付ける値を指定してください。例: `"block"`, `"underline"`, `"bar"` |
| `terminal.scrollback`         | number  | `10000`                                | ターミナルのスクロールバック行数です。値を大きくすると過去ログを多く保持できますが、メモリ使用量が増える可能性があります。                                      |
| `terminal.auto_session_log`   | boolean | `false`                                | `true` にすると、SSH/シリアル/Telnet 接続のターミナル入出力を平文ログとして保存します。                                                                         |
| `terminal.log_format`         | string  | `"display"`                            | セッションログの整形方式です。`"display"` は画面表示に近い内容、`"strip_controls"` は制御文字を除去した内容を保存します。                                       |
| `terminal.include_log_header` | boolean | `true`                                 | `true` にすると、新しいセッションログの先頭に ExaTerm ヘッダとして種別、接続先、ログモード、開始時刻を記録します。                                              |

### セッションログの注意

`terminal.auto_session_log` を `true` にすると、ターミナルに表示された内容や入力内容がログファイルに保存されます。ログには次のような機密情報が含まれる可能性があります。

- コマンドと実行結果
- ホスト名、ユーザー名、プロンプト
- ネットワーク機器やサーバーの出力
- 誤って入力したトークン、パスワード、秘密情報

ログは通常、次の場所に保存されます。

```text
%AppData%\ExaTerm\logs
```

機密性の高い環境では、必要な場合のみ有効にしてください。

`terminal.log_format` が `"display"` の場合、Backspace、カーソル左移動、行末消去などの一般的な行編集を反映してからログへ保存します。`"strip_controls"` の場合は制御シーケンスを除去しますが、編集途中の文字が残る場合があります。

`terminal.include_log_header` が `false` の場合、新しい自動ログと手動ログは ExaTerm ヘッダを書かず、ターミナル内容から直接始まります。既存のログファイルは変更されません。

## ssh

| パラメータ                    | 型      | 既定値  | 説明                                                                                                                  |
| ----------------------------- | ------- | ------- | --------------------------------------------------------------------------------------------------------------------- |
| `ssh.allow_legacy_algorithms` | boolean | `false` | `true` にすると、現代的な SSH 既定アルゴリズムに対応していない古い機器向けに、レガシー SSH アルゴリズムも提示します。 |

### レガシー SSH アルゴリズムの注意

古い SSH サーバーやネットワーク機器へ接続する必要がある場合を除き、`ssh.allow_legacy_algorithms` は `false` のままにしてください。有効にすると、SHA-1 ベースの鍵交換、CBC/3DES 暗号、`ssh-rsa` ホスト鍵など、強度の低い互換アルゴリズムを許可します。

### 利用可能な SSH アルゴリズム

ExaTerm は次の SSH アルゴリズムを提示します。レガシー追加分は、`ssh.allow_legacy_algorithms` を `true` にした場合のみ提示されます。

| カテゴリ | 既定で利用可能                                                                                                                               | レガシー追加分                                                                           |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| 鍵交換   | `curve25519-sha256`, `curve25519-sha256@libssh.org`, `diffie-hellman-group16-sha512`, `diffie-hellman-group14-sha256`                        | `diffie-hellman-group1-sha1`, `diffie-hellman-group14-sha1`                              |
| 暗号     | `chacha20-poly1305@openssh.com`, `aes256-gcm@openssh.com`, `aes256-ctr`, `aes192-ctr`, `aes128-ctr`                                          | `aes128-cbc`, `aes192-cbc`, `aes256-cbc`, `3des-cbc`                                     |
| MAC      | `hmac-sha2-512-etm@openssh.com`, `hmac-sha2-256-etm@openssh.com`, `hmac-sha2-512`, `hmac-sha2-256`, `hmac-sha1-etm@openssh.com`, `hmac-sha1` | 追加される MAC アルゴリズムはありません。現在の既定値に `hmac-sha1` 系が含まれています。 |
| ホスト鍵 | `ssh-ed25519`, `ecdsa-sha2-nistp256`, `ecdsa-sha2-nistp521`, `rsa-sha2-256`, `rsa-sha2-512`                                                  | `ssh-rsa`                                                                                |

Strict key exchange や extension info などの SSH 内部拡張マーカーは、ユーザーが直接設定するものではないため、この一覧には含めていません。

## saved_connections

`saved_connections` は保存済み SSH/Telnet 接続プロファイルを表す配列です。プロファイルは接続ダイアログから管理できます。シリアルのプロファイルは現状非対応です。パスワード、秘密鍵本文、鍵パスフレーズ、その他の認証情報はこのセクションには保存されません。プロファイルのメモは平文で保存され、MCP のプロファイル接続が有効な場合は `list_connection_profiles` で返る可能性があるため、秘密情報は入力しないでください。

| パラメータ         | 型                 | 説明                                                                                                                                                                |
| ------------------ | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id`               | string             | プロファイル名兼識別子です。                                                                                                                                        |
| `connection_type`  | string             | 接続種別です。プロファイルで対応している値は `"ssh"` と `"telnet"` です。                                                                                           |
| `host`             | string または null | SSH または Telnet 接続先ホストです。                                                                                                                                |
| `port`             | number または null | SSH または Telnet 接続先ポートです。                                                                                                                                |
| `username`         | string または null | SSH ユーザー名です。Telnet プロファイルでは使用しません。                                                                                                           |
| `encoding`         | string または null | このプロファイルで接続したときのターミナル表示文字コードです。指定できる値は `"utf-8"`、`"shift-jis"`、`"euc-jp"` です。未設定の場合は `"utf-8"` として扱われます。 |
| `terminal_mode`    | string または null | このプロファイルで接続したときのターミナルモードです。指定できる値は `"general"` と `"cisco_ios"` です。未設定の場合は `"general"` として扱われます。               |
| `auth_method`      | string または null | SSH 認証方式です。指定できる値は `"password"` と `"public_key"` です。未設定の場合は `"password"` として扱われます。Telnet プロファイルでは使用しません。           |
| `private_key_path` | string または null | SSH の `"public_key"` 認証で使用する秘密鍵ファイルのパスです。例: `id_ed25519`。ファイル本文とパスフレーズは保存されません。                                        |
| `jump_profile_id`  | string または null | SSH 踏み台プロファイルの ID です。参照先は保存済み SSH プロファイルである必要があります。踏み台は 1 段のみ対応し、多段指定は拒否されます。                          |
| `memo`             | string または null | 任意の平文メモです。機種名、用途、作業時の注意などを記録できます。空でないメモは MCP のプロファイル一覧で返る場合があります。                                       |

例:

```json
{
  "id": "dev-server",
  "connection_type": "ssh",
  "host": "192.168.1.10",
  "port": 22,
  "username": "admin",
  "auth_method": "public_key",
  "private_key_path": "C:\\Users\\user\\.ssh\\id_ed25519",
  "jump_profile_id": "bastion",
  "encoding": "shift-jis",
  "terminal_mode": "cisco_ios",
  "memo": "Cisco ISR branch edge"
}
```

`jump_profile_id` を設定すると、ExaTerm は参照先の SSH プロファイルへ先に接続し、その踏み台経由で接続先への SSH 接続を開きます。踏み台プロファイルからさらに別の踏み台を参照することはできず、自分自身を踏み台に指定することもできません。踏み台と接続先の SSH パスワードや暗号化鍵パスフレーズは ExaTerm UI で入力し、`config.json` には保存されません。

Telnet の例:

```json
{
  "id": "legacy-router",
  "connection_type": "telnet",
  "host": "192.168.1.20",
  "port": 23,
  "encoding": "euc-jp",
  "terminal_mode": "cisco_ios",
  "memo": "Legacy access switch"
}
```

## よくある変更例

### 表示言語を日本語にする

```json
"language": "ja"
```

### Ollama を有効にする

```json
"ai": {
  "ollama_enabled": true,
  "ollama_base_url": "http://localhost:11434",
  "default_provider": "Ollama",
  "default_model": "llama3.1",
  "debug_log_enabled": false
}
```

`default_model` には、Ollama にインストール済みのモデル名を指定してください。

### Azure OpenAI を有効にする

```json
"ai": {
  "azure_openai_enabled": true,
  "azure_openai_endpoint": "https://your-resource.openai.azure.com/openai/v1/chat/completions",
  "azure_openai_deployment": "my-gpt4o",
  "default_provider": "AzureOpenAi",
  "default_model": "my-gpt4o",
  "debug_log_enabled": false
}
```

Azure OpenAI API キーは設定画面から保存してください。ExaTerm は設定された Endpoint URL に、変更を加えずリクエストを送信します。

### OpenRouter を使用する

```json
"ai": {
  "default_provider": "OpenRouter",
  "default_model": "openai/gpt-4o",
  "debug_log_enabled": false
}
```

OpenRouter API キーは設定画面から保存してください。ExaTerm は OpenRouter から利用可能なモデル一覧を取得し、一覧を読み込めない場合は代表的なモデル ID にフォールバックします。

### セッションログを無効にする

```json
"terminal": {
  "auto_session_log": false
}
```

実際の `terminal` オブジェクトには他の項目も残してください。上記は変更箇所だけを示した例です。

### レガシー SSH アルゴリズムを許可する

```json
"ssh": {
  "allow_legacy_algorithms": true
}
```

既定の SSH アルゴリズムでは接続できない古い機器に限って使用してください。

## トラブルシューティング

| 症状                         | 対処                                                                                                                                                                                                                                                                                                     |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ExaTerm が設定を読み込めない | JSON の構文を確認してください。特に余分なカンマ、引用符、波括弧の不足を確認します。                                                                                                                                                                                                                      |
| 設定を変更しても反映されない | ExaTerm を再起動するか、設定画面で保存し直してください。                                                                                                                                                                                                                                                 |
| AI プロバイダが表示されない  | クラウド系プロバイダは API キー登録が必要です。Azure OpenAI は `azure_openai_enabled`、`azure_openai_endpoint`、`azure_openai_deployment` も確認してください。OpenRouter は設定画面で OpenRouter API キーを保存してください。Ollama は `ollama_enabled` と Ollama サーバーの起動状態を確認してください。 |
| 古い SSH 機器に接続できない  | 共通の SSH アルゴリズムがないことを示すエラーの場合は、`ssh.allow_legacy_algorithms` を `true` にして再接続してください。不要になったら無効に戻してください。                                                                                                                                            |
| 文字が見づらい               | `terminal.font_size` または `terminal.font_family` を調整してください。                                                                                                                                                                                                                                  |
| ログを残したくない           | `terminal.auto_session_log` を `false` にしてください。既に作成済みのログは必要に応じて `%AppData%\ExaTerm\logs` から削除してください。                                                                                                                                                                  |
