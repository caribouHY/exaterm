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
  "config_version": 6,
  "language": "system",
  "updates": {
    "check_on_startup": true
  },
  "connection_history": {
    "enabled": true
  },
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
  "external_control": {
    "enabled": false,
    "connect_enabled": false,
    "mcp_enabled": false,
    "cli_enabled": false
  },
  "shortcuts": {
    "new_connection": { "key": "n", "ctrl": true, "alt": false, "shift": false },
    "new_window": { "key": "n", "ctrl": true, "alt": false, "shift": true },
    "open_settings": { "key": ",", "ctrl": true, "alt": false, "shift": false },
    "exit": null,
    "terminal_select_all": { "key": "a", "ctrl": true, "alt": false, "shift": true },
    "terminal_copy": { "key": "c", "ctrl": true, "alt": false, "shift": true },
    "terminal_paste": { "key": "v", "ctrl": true, "alt": false, "shift": true },
    "terminal_log_start_overwrite": { "key": "F9", "ctrl": true, "alt": false, "shift": true },
    "terminal_log_start_append": null,
    "terminal_log_stop": { "key": "F10", "ctrl": true, "alt": false, "shift": true },
    "terminal_log_pause": null,
    "terminal_log_resume": null
  },
  "terminal": {
    "font_size": 14,
    "font_family": "Consolas, 'Courier New', monospace",
    "cursor_style": "block",
    "scrollback": 10000,
    "auto_session_log": false,
    "log_format": "display",
    "include_log_header": false
  },
  "ssh": {
    "algorithm_mode": "default",
    "algorithms": {
      "kex": [],
      "host_key": [],
      "cipher": [],
      "mac": [],
      "compression": []
    }
  },
  "saved_connections": []
}
```

## ルート項目

| パラメータ           | 型     | 既定値     | 説明                                                                                                                                            |
| -------------------- | ------ | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `config_version`     | number | `6`        | 設定ファイルのバージョンです。通常は変更しません。古い設定を読み込んだ場合、ExaTerm が現在のバージョンへ更新します。                            |
| `language`           | string | `"system"` | 画面表示言語です。`"system"` は OS の言語設定に従います。`"en"` は英語、`"ja"` は日本語です。未対応のシステム言語は英語にフォールバックします。 |
| `updates`            | object | 下記参照   | 公開済みのExaTerm正式版を自動確認する動作を設定します。                                                                                         |
| `connection_history` | object | 下記参照   | ローカルのSSH/Telnet接続履歴を設定します。                                                                                                      |
| `ai`                 | object | 下記参照   | AI アシスタント関連の設定です。                                                                                                                 |
| `external_control`   | object | 下記参照   | ターミナル CLI と MCP 互換アダプターのためのローカル外部制御設定です。                                                                          |
| `shortcuts`          | object | 下記参照   | アプリケーションのキーボードショートカット設定です。                                                                                            |
| `terminal`           | object | 下記参照   | ターミナル表示とログ関連の設定です。                                                                                                            |
| `ssh`                | object | 下記参照   | SSH 接続の互換性設定です。                                                                                                                      |
| `saved_connections`  | array  | `[]`       | 保存済み SSH/Telnet 接続プロファイルです。プロファイルは接続ダイアログから作成、選択、削除できます。                                            |

## updates

| キー               | 型      | 既定値 | 説明                                                                                                                         |
| ------------------ | ------- | ------ | ---------------------------------------------------------------------------------------------------------------------------- |
| `check_on_startup` | boolean | `true` | ExaTermのmainウィンドウ起動時に、最新の公開済み正式版を1回確認します。ダウンロードとインストールには引き続き確認が必要です。 |

この値が`false`でも、アプリメニューからの手動確認は利用できます。

## connection_history

| キー      | 型      | 既定値 | 説明                                                                                                                             |
| --------- | ------- | ------ | -------------------------------------------------------------------------------------------------------------------------------- |
| `enabled` | boolean | `true` | SSH/Telnet接続が成功したときに設定を保存します。オフにすると新規記録を停止しますが、既存の履歴は削除するまで表示・利用できます。 |

接続履歴は `config.json` とは別に次の場所へ保存されます。

```text
%AppData%\ExaTerm\connection_history.json
```

ホスト、ポート、SSHユーザー名、認証方式、秘密鍵パス、踏み台プロファイルID、文字コード、ターミナルモード、接続日時を平文で保存します。パスワード、秘密鍵のパスフレーズ、踏み台の認証情報は保存しません。SSH/Telnetそれぞれ最大10件を保持します。接続ダイアログから個別削除でき、設定画面からすべて削除できます。

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

## external_control

| パラメータ                         | 型      | 既定値  | 説明                                                                                                                                                                                                                        |
| ---------------------------------- | ------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `external_control.enabled`         | boolean | `false` | ローカル外部制御アクセスのマスター許可です。`true` にしても各インターフェースは自動で有効にならないため、`external_control.cli_enabled` または `external_control.mcp_enabled` を別途有効化します。                          |
| `external_control.connect_enabled` | boolean | `false` | `true` にすると、信頼済み外部制御クライアントが保存済み SSH/Telnet プロファイルやシリアルコンソールを開けます。SSH 認証情報は ExaTerm UI で入力し、外部クライアントへは公開されず保存もされません。                         |
| `external_control.mcp_enabled`     | boolean | `false` | `external_control.enabled=true` と併用して `true` にすると、ローカル MCP クライアントから `exaterm-mcp` 互換アダプターを使用できます。                                                                                      |
| `external_control.cli_enabled`     | boolean | `false` | `external_control.enabled=true` と併用して `true` にすると、信頼済みローカルプログラムが `exaterm-cli` から共通のターミナル操作を呼び出し、JSON 結果を取得できます。詳細は[CLI ガイド](CLI_GUIDE.ja.md)を参照してください。 |

古い設定ファイルには、従来の `mcp` オブジェクトや `saved_connections[*].mcp_enabled` が残っている場合があります。ExaTerm は読み込み時にそれらを自動移行します。新旧設定が併存する場合は、新しい `external_control` 側を優先し、旧設定は不足値の補完だけに使います。ExaTerm が設定を保存すると、新しい `external_control` と `external_control_enabled` だけが残ります。

HTTP MCP transport は削除されました。古い設定ファイルに残っている HTTP 専用の `mcp.host` と `mcp.port` の値は無視され、次に ExaTerm が設定を保存したときに出力されなくなります。以前 HTTP MCP を使用していた場合は、`external_control.enabled=true` と `external_control.mcp_enabled=true` を手動で設定し、MCP クライアント側で `exaterm-mcp` を起動するよう設定してください。

### MCP ツール

MCP が有効な場合、外部クライアントは次のツールを呼び出せます。

- `list_terminal_sessions`: ユーザーが ExaTerm で開いたターミナルセッションを一覧表示します。
- `read_terminal_output`: 必須の `mode` 引数に応じてセッション出力を読み取るか待機します。
  - `recent`: 保持されている直近出力を即時に読み取ります。
  - `delta`: 必須の `cursor` 以降の出力を即時に読み取ります。
  - `wait`: 新しい出力、または任意の `contains` 文字列が現れるまで待機します。`cursor` を省略した場合は現在の出力位置から待機します。
- `send_terminal_input`: 接続中のセッションへテキストを送信します。
- `run_terminal_command`: 接続中のセッションへコマンドを送信し、出力待機後に差分出力を返します。
- `start_terminal_log`: 接続中セッションの手動平文ログを開始します。ログは `%AppData%\ExaTerm\logs` 配下に保存され、返却値には作成されたファイルパスが含まれます。
- `stop_terminal_log`: ExaTerm が表示済み出力をログへ flush した後、セッションの手動平文ログを停止します。

出力読み取りの例:

```json
{ "session_id": "session-id", "mode": "recent", "max_chars": 2000 }
```

```json
{ "session_id": "session-id", "mode": "delta", "cursor": 1200 }
```

```json
{
  "session_id": "session-id",
  "mode": "wait",
  "cursor": 1200,
  "contains": "router#",
  "timeout_ms": 30000
}
```

`external_control.connect_enabled` も `true` の場合、外部クライアントは次の追加ツールを呼び出せます。

- `list_connection_profiles`: 個別に外部制御利用を許可した保存済み SSH/Telnet プロファイルを一覧表示します。秘密鍵パスや認証情報は返しません。
- `connect_saved_profile`: 保存済みプロファイルから新しい SSH/Telnet セッションを開きます。`profile_id` と `connection_type`（`"ssh"` または `"telnet"`）の両方が必須なので、異なる種別で同じ ID を使用できます。SSH 認証情報は ExaTerm UI で入力します。
- `list_serial_ports`: 現在利用可能なシリアルポートを一覧表示します。
- `connect_serial_console`: MCP ツール引数で指定したポート名と通信設定から新しいシリアルコンソールセッションを開き、ExaTerm のタブとして表示します。ポート名は利用可能なシリアルポートと完全一致する必要があります。

`read_terminal_output` は選択された `mode` を返します。また、`read_terminal_output` と `run_terminal_command` は `start_cursor`、`cursor`、`truncated` も返します。返された `cursor` を `mode: "delta"` または `mode: "wait"` の `read_terminal_output` に渡すと、同じ位置から読み取りを継続できます。古い出力が内部バッファから切り詰められている場合、`truncated` は `true` になります。`wait` の結果には `matched` と `timed_out` も含まれます。

`wait` モードと `run_terminal_command` の待機時間は最大 60 秒です。`run_terminal_command` は既存の接続済みセッションだけを対象とします。保存済みプロファイルからの新規接続には、明示的に有効化した `connect_saved_profile` を使用します。

従来の `read_terminal_output_delta` と `wait_terminal_output` は削除されました。それぞれ `mode: "delta"` と `mode: "wait"` を指定した `read_terminal_output` に置き換えてください。

MCP 互換アダプターと CLI は保存済み認証情報の読み取り、API キーの公開、ログファイル本文の直接読み取りを行いません。外部クライアントがログを開始・停止した場合も、受け取るのはログ状態とファイルパスだけです。SSH/Telnet 新規接続は保存済みプロファイルに限定され、シリアル接続は利用可能なポート名だけを対象にし、すべての外部新規接続には `external_control.connect_enabled=true` が必要です。プロファイルは `saved_connections[*].external_control_enabled=false` で除外できます。SSH の known_hosts 検証と ExaTerm UI での認証入力は維持されます。機密情報を含む可能性があるため、外部制御は信頼済みローカルクライアントに対してのみ有効化してください。

## shortcuts

各ショートカットは `key`、`ctrl`、`alt`、`shift` を持つオブジェクトです。未割り当てにする場合は `null` を指定します。通常キーと `Space` には `ctrl` または `alt` が必要です。`F1`～`F12` は修飾キーなしでも割り当てられます。割り当ての重複は許可されず、Windows が使用する `Alt+F4` は予約されています。

| パラメータ                               | 既定値           | 操作                                                                             |
| ---------------------------------------- | ---------------- | -------------------------------------------------------------------------------- |
| `shortcuts.new_connection`               | `Ctrl+N`         | 新規接続ダイアログを開きます。                                                   |
| `shortcuts.new_window`                   | `Ctrl+Shift+N`   | 新しい ExaTerm ウィンドウを開きます。                                            |
| `shortcuts.open_settings`                | `Ctrl+,`         | ショートカットなどのアプリ設定を開きます。                                       |
| `shortcuts.exit`                         | 未割り当て       | ExaTerm を終了します。接続中のターミナルセッションがある場合は確認します。       |
| `shortcuts.terminal_select_all`          | `Ctrl+Shift+A`   | ターミナルの画面とスクロールバック全体を選択します。                             |
| `shortcuts.terminal_copy`                | `Ctrl+Shift+C`   | ターミナルで選択した文字をコピーします。                                         |
| `shortcuts.terminal_paste`               | `Ctrl+Shift+V`   | 接続中のターミナルへクリップボードの文字をペーストします。                       |
| `shortcuts.terminal_log_start_overwrite` | `Ctrl+Shift+F9`  | 保存ダイアログを開き、新しい手動ログを開始するか選択したファイルを上書きします。 |
| `shortcuts.terminal_log_start_append`    | 未割り当て       | 保存ダイアログを開き、選択した手動ログファイルへ追記します。                     |
| `shortcuts.terminal_log_stop`            | `Ctrl+Shift+F10` | 保留中の画面表示をflushして、実行中の手動ログを停止します。                      |
| `shortcuts.terminal_log_pause`           | 未割り当て       | ターミナルセッションで実行中の自動ログと手動ログを一時停止します。               |
| `shortcuts.terminal_log_resume`          | 未割り当て       | 一時停止中の自動ログと手動ログを再開します。                                     |

英字キーは小文字、空白キーは `"Space"`、ファンクションキーは `"F2"` のような大文字表記で保存されます。修飾キーは完全一致で判定するため、たとえば `Ctrl+Shift+N` は `Ctrl+N` としては実行されません。

ターミナル用ショートカットは、ターミナルにキーボードフォーカスがある場合だけ動作します。`Ctrl+A`、`Ctrl+C`、`Ctrl+V`をアプリまたはターミナル操作へ割り当てると、ExaTermにフォーカスがある間は、行頭移動、中断、quoted insertなどリモート側で一般的な操作より割り当てた操作が優先されます。

## terminal

| パラメータ                    | 型      | 既定値                                 | 説明                                                                                                                                                            |
| ----------------------------- | ------- | -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `terminal.font_size`          | number  | `14`                                   | ターミナルのフォントサイズです。設定画面では `8` から `32` の範囲で指定できます。                                                                               |
| `terminal.font_family`        | string  | `"Consolas, 'Courier New', monospace"` | ターミナルのフォントファミリーです。CSS の `font-family` と同じ形式で指定します。                                                                               |
| `terminal.cursor_style`       | string  | `"block"`                              | ターミナルのカーソル形状です。現在の既定値はブロックカーソルです。手動編集時は xterm.js が受け付ける値を指定してください。例: `"block"`, `"underline"`, `"bar"` |
| `terminal.scrollback`         | number  | `10000`                                | ターミナルのスクロールバック行数です。値を大きくすると過去ログを多く保持できますが、メモリ使用量が増える可能性があります。                                      |
| `terminal.auto_session_log`   | boolean | `false`                                | `true` にすると、SSH/シリアル/Telnet 接続のターミナル入出力を平文ログとして保存します。                                                                         |
| `terminal.log_format`         | string  | `"display"`                            | セッションログの整形方式です。`"display"` は画面表示に近い内容、`"strip_controls"` は制御文字を除去した内容を保存します。                                       |
| `terminal.include_log_header` | boolean | `false`                                | `true` にすると、新しいセッションログの先頭に ExaTerm ヘッダとして種別、接続先、ログモード、開始時刻を記録します。                                              |

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

| パラメータ                   | 型     | 既定値      | 説明                                                                             |
| ---------------------------- | ------ | ----------- | -------------------------------------------------------------------------------- |
| `ssh.algorithm_mode`         | 文字列 | `"default"` | 推奨設定は `"default"`、設定した許可リストを使う場合は `"custom"` を指定します。 |
| `ssh.algorithms.kex`         | 配列   | `[]`        | カスタム設定で許可する鍵交換アルゴリズムです。                                   |
| `ssh.algorithms.host_key`    | 配列   | `[]`        | カスタム設定で許可するサーバーホスト鍵アルゴリズムです。                         |
| `ssh.algorithms.cipher`      | 配列   | `[]`        | カスタム設定で許可する共通鍵暗号です。                                           |
| `ssh.algorithms.mac`         | 配列   | `[]`        | カスタム設定で許可するメッセージ認証アルゴリズムです。                           |
| `ssh.algorithms.compression` | 配列   | `[]`        | カスタム設定で許可する圧縮アルゴリズムです。                                     |

### 利用可能な SSH アルゴリズム

設定画面には、同梱する SSH ライブラリが対応しているアルゴリズム一覧を表示します。カスタム設定では、すべての分類で1件以上選択してください。JSON 配列の記載順ではなく、ExaTerm の組み込み優先順でネゴシエーションします。SHA-1 鍵交換、CBC/3DES 暗号、`ssh-rsa` などは互換用として表示します。

未知の名前、重複、空の分類は拒否されます。Strict key exchange や extension info などの SSH 内部拡張マーカーは自動管理されるため、設定には追加しないでください。

## saved_connections

`saved_connections` は保存済み SSH/Telnet 接続プロファイルを表す配列です。プロファイルは接続ダイアログから管理できます。シリアルのプロファイルは現状非対応です。パスワード、秘密鍵本文、鍵パスフレーズ、その他の認証情報はこのセクションには保存されません。プロファイルのメモは平文で保存され、外部プロファイル接続が有効な場合は `list_connection_profiles` で返る可能性があるため、秘密情報は入力しないでください。既存プロファイルで `external_control_enabled` が未設定の場合は `true` として扱われます。

| パラメータ                 | 型                 | 説明                                                                                                                                                                                                                                             |
| -------------------------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `id`                       | string             | プロファイル名兼識別子です。                                                                                                                                                                                                                     |
| `connection_type`          | string             | 接続種別です。プロファイルで対応している値は `"ssh"` と `"telnet"` です。                                                                                                                                                                        |
| `host`                     | string または null | SSH または Telnet 接続先ホストです。                                                                                                                                                                                                             |
| `port`                     | number または null | SSH または Telnet 接続先ポートです。                                                                                                                                                                                                             |
| `username`                 | string または null | SSH ユーザー名です。Telnet プロファイルでは使用しません。                                                                                                                                                                                        |
| `encoding`                 | string または null | このプロファイルで接続したときのターミナル表示文字コードです。指定できる値は `"utf-8"`、`"shift-jis"`、`"euc-jp"` です。未設定の場合は `"utf-8"` として扱われます。                                                                              |
| `terminal_mode`            | string または null | このプロファイルで接続したときのターミナルモードです。指定できる値は `"general"`、`"cisco_ios"`、`"arista_eos"`、`"vyos"`、`"fujitsu_sir"`、`"allied_telesis_awplus"`、`"furukawa_fitelnet"` です。未設定の場合は `"general"` として扱われます。 |
| `auth_method`              | string または null | SSH 認証方式です。指定できる値は `"password"` と `"public_key"` です。未設定の場合は `"password"` として扱われます。Telnet プロファイルでは使用しません。                                                                                        |
| `private_key_path`         | string または null | SSH の `"public_key"` 認証で使用する秘密鍵ファイルのパスです。例: `id_ed25519`。ファイル本文とパスフレーズは保存されません。                                                                                                                     |
| `jump_profile_id`          | string または null | SSH 踏み台プロファイルの ID です。参照先は保存済み SSH プロファイルである必要があります。踏み台は 1 段のみ対応し、多段指定は拒否されます。                                                                                                       |
| `memo`                     | string または null | 任意の平文メモです。機種名、用途、作業時の注意などを記録できます。空でないメモは外部制御クライアント向けのプロファイル一覧で返る場合があります。                                                                                                 |
| `external_control_enabled` | boolean            | 信頼済み CLI / MCP クライアントがこの保存済みプロファイルを一覧表示し、接続に使えるかどうかです。未設定時の既定値は `true` です。                                                                                                                |

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

### 表示言語をシステム設定に従わせる

```json
"language": "system"
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

### SSH アルゴリズムを選択する

```json
"ssh": {
  "algorithm_mode": "custom",
  "algorithms": {
    "kex": ["curve25519-sha256", "diffie-hellman-group14-sha1"],
    "host_key": ["ssh-ed25519", "ssh-rsa"],
    "cipher": ["aes256-ctr", "aes128-cbc"],
    "mac": ["hmac-sha2-256", "hmac-sha1"],
    "compression": ["none"]
  }
}
```

カスタム設定は設定画面から作成することを推奨します。既存の `allow_legacy_algorithms` 設定は自動的に移行されます。

## トラブルシューティング

| 症状                         | 対処                                                                                                                                                                                                                                                                                                     |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ExaTerm が設定を読み込めない | JSON の構文を確認してください。特に余分なカンマ、引用符、波括弧の不足を確認します。                                                                                                                                                                                                                      |
| 設定を変更しても反映されない | ExaTerm を再起動するか、設定画面で保存し直してください。                                                                                                                                                                                                                                                 |
| AI プロバイダが表示されない  | クラウド系プロバイダは API キー登録が必要です。Azure OpenAI は `azure_openai_enabled`、`azure_openai_endpoint`、`azure_openai_deployment` も確認してください。OpenRouter は設定画面で OpenRouter API キーを保存してください。Ollama は `ollama_enabled` と Ollama サーバーの起動状態を確認してください。 |
| 古い SSH 機器に接続できない  | 共通の SSH アルゴリズムがないことを示すエラーの場合は、設定画面でカスタム選択に切り替え、その機器に必要な互換用アルゴリズムだけを有効にしてください。                                                                                                                                                    |
| 文字が見づらい               | `terminal.font_size` または `terminal.font_family` を調整してください。                                                                                                                                                                                                                                  |
| ログを残したくない           | `terminal.auto_session_log` を `false` にしてください。既に作成済みのログは必要に応じて `%AppData%\ExaTerm\logs` から削除してください。                                                                                                                                                                  |
