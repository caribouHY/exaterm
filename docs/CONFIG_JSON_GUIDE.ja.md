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
    "host": "127.0.0.1",
    "port": 8765
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
| `mcp`               | object | 下記参照 | 外部 AI エージェントからターミナルを制御するためのローカル MCP サーバー設定です。                                    |
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

| パラメータ    | 型      | 既定値        | 説明                                                                                                                                                                           |
| ------------- | ------- | ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `mcp.enabled` | boolean | `false`       | `true` にすると、外部 AI エージェント向けにローカル MCP Streamable HTTP エンドポイント `/mcp` を起動します。ExaTerm 上でユーザーが開いたターミナルセッションだけを公開します。 |
| `mcp.host`    | string  | `"127.0.0.1"` | MCP サーバーの待ち受けアドレスです。他の端末からターミナル制御を受け付けるリスクを理解している場合を除き、ループバックアドレスのままにしてください。                           |
| `mcp.port`    | number  | `8765`        | MCP サーバーの TCP ポートです。ポートが使用中の場合、MCP サーバーの起動に失敗しますが、ExaTerm 本体は起動を続けます。                                                          |

### MCP ツール

MCP が有効な場合、外部クライアントは次のツールを呼び出せます。

- `list_terminal_sessions`: ユーザーが ExaTerm で開いたターミナルセッションを一覧表示します。
- `read_terminal_output`: セッションの直近出力を読み取ります。
- `send_terminal_input`: 接続中のセッションへテキストを送信します。

MCP サーバーは新規接続の作成、保存済み認証情報の読み取り、API キーの公開、ログファイルの直接読み取りを行いません。ただしターミナル出力自体に機密情報が含まれる可能性があるため、信頼できるローカルクライアントに対してのみ有効化してください。

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

`saved_connections` は保存済み SSH/Telnet 接続プロファイルを表す配列です。プロファイルは接続ダイアログから管理できます。シリアルのプロファイルは現状非対応です。パスワード、秘密鍵本文、鍵パスフレーズ、その他の認証情報はこのセクションには保存されません。

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
  "encoding": "shift-jis",
  "terminal_mode": "cisco_ios"
}
```

Telnet の例:

```json
{
  "id": "legacy-router",
  "connection_type": "telnet",
  "host": "192.168.1.20",
  "port": 23,
  "encoding": "euc-jp",
  "terminal_mode": "cisco_ios"
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
