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
    "ollama_enabled": false,
    "ollama_base_url": "http://localhost:11434",
    "default_provider": "OpenAi",
    "default_model": "gpt-4o"
  },
  "terminal": {
    "font_size": 14,
    "font_family": "Consolas, 'Courier New', monospace",
    "cursor_style": "block",
    "scrollback": 10000,
    "auto_session_log": false
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
| `terminal`          | object | 下記参照 | ターミナル表示とログ関連の設定です。                                                                                 |
| `saved_connections` | array  | `[]`     | 保存済み接続情報です。現状の設定画面では主に内部データとして扱われます。                                             |

## ai

| パラメータ            | 型      | 既定値                     | 説明                                                                                                                                                                                                    |
| --------------------- | ------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ai.ollama_enabled`   | boolean | `false`                    | `true` にすると、Ollama のモデルを AI パネルに表示します。Ollama を使用するには、ローカルまたは指定 URL の Ollama サーバーが起動している必要があります。                                                |
| `ai.ollama_base_url`  | string  | `"http://localhost:11434"` | Ollama API のベース URL です。ローカル環境の標準設定では `"http://localhost:11434"` を使用します。空文字の場合、画面上では既定 URL として扱われます。                                                   |
| `ai.default_provider` | string  | `"OpenAi"`                 | AI パネルで優先的に選択されるプロバイダです。使用可能な値は `"OpenAi"`, `"Anthropic"`, `"Gemini"`, `"Ollama"` です。                                                                                    |
| `ai.default_model`    | string  | `"gpt-4o"`                 | AI パネルで優先的に選択されるモデル ID です。設定画面からは現在直接編集できないため、必要な場合は手動で編集します。保存したモデルが利用できない場合は、利用可能なモデルへ自動的にフォールバックします。 |

### AI API キーについて

OpenAI、Anthropic、Google Gemini の API キーは `config.json` には保存されません。設定画面で登録したキーは、OS の資格情報ストアに保存されます。

Ollama は通常 API キーを必要としません。`ai.ollama_enabled` と `ai.ollama_base_url` を設定してください。

### 代表的なモデル ID

| プロバイダ | モデル ID の例                                          |
| ---------- | ------------------------------------------------------- |
| OpenAI     | `gpt-4o`, `gpt-4o-mini`                                 |
| Anthropic  | `claude-sonnet-4-20250514`, `claude-3-5-haiku-20241022` |
| Gemini     | `gemini-2.5-pro`, `gemini-2.5-flash`                    |
| Ollama     | ローカルの Ollama にインストール済みのモデル名          |

## terminal

| パラメータ                  | 型      | 既定値                                 | 説明                                                                                                                                                            |
| --------------------------- | ------- | -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `terminal.font_size`        | number  | `14`                                   | ターミナルのフォントサイズです。設定画面では `8` から `32` の範囲で指定できます。                                                                               |
| `terminal.font_family`      | string  | `"Consolas, 'Courier New', monospace"` | ターミナルのフォントファミリーです。CSS の `font-family` と同じ形式で指定します。                                                                               |
| `terminal.cursor_style`     | string  | `"block"`                              | ターミナルのカーソル形状です。現在の既定値はブロックカーソルです。手動編集時は xterm.js が受け付ける値を指定してください。例: `"block"`, `"underline"`, `"bar"` |
| `terminal.scrollback`       | number  | `10000`                                | ターミナルのスクロールバック行数です。値を大きくすると過去ログを多く保持できますが、メモリ使用量が増える可能性があります。                                      |
| `terminal.auto_session_log` | boolean | `false`                                | `true` にすると、SSH/シリアル接続のターミナル入出力を平文ログとして保存します。                                                                                 |

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

## saved_connections

`saved_connections` は保存済み接続を表す配列です。各要素には次の項目があります。

| パラメータ        | 型                 | 説明                                                  |
| ----------------- | ------------------ | ----------------------------------------------------- |
| `id`              | string             | 接続情報の識別子です。                                |
| `name`            | string             | 接続名です。                                          |
| `connection_type` | string             | 接続種別です。通常は `"ssh"` または `"serial"` です。 |
| `host`            | string または null | SSH 接続先ホストです。                                |
| `port`            | number または null | SSH 接続先ポートです。                                |
| `username`        | string または null | SSH ユーザー名です。                                  |
| `serial_port`     | string または null | シリアルポート名です。                                |
| `baud_rate`       | number または null | シリアル通信のボーレートです。                        |

例:

```json
{
  "id": "dev-server",
  "name": "開発サーバー",
  "connection_type": "ssh",
  "host": "192.168.1.10",
  "port": 22,
  "username": "admin",
  "serial_port": null,
  "baud_rate": null
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
  "default_model": "llama3.1"
}
```

`default_model` には、Ollama にインストール済みのモデル名を指定してください。

### セッションログを無効にする

```json
"terminal": {
  "auto_session_log": false
}
```

実際の `terminal` オブジェクトには他の項目も残してください。上記は変更箇所だけを示した例です。

## トラブルシューティング

| 症状                         | 対処                                                                                                                                    |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| ExaTerm が設定を読み込めない | JSON の構文を確認してください。特に余分なカンマ、引用符、波括弧の不足を確認します。                                                     |
| 設定を変更しても反映されない | ExaTerm を再起動するか、設定画面で保存し直してください。                                                                                |
| AI プロバイダが表示されない  | クラウド系プロバイダは API キー登録が必要です。Ollama は `ollama_enabled` と Ollama サーバーの起動状態を確認してください。              |
| 文字が見づらい               | `terminal.font_size` または `terminal.font_family` を調整してください。                                                                 |
| ログを残したくない           | `terminal.auto_session_log` を `false` にしてください。既に作成済みのログは必要に応じて `%AppData%\ExaTerm\logs` から削除してください。 |
