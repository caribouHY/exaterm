use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendLanguage {
    #[default]
    English,
    Japanese,
}

impl BackendLanguage {
    pub fn from_app_language(language: &str) -> Self {
        if language.trim().eq_ignore_ascii_case("ja") || language.trim().starts_with("ja-") {
            Self::Japanese
        } else {
            Self::English
        }
    }
}

#[derive(Default)]
pub struct BackendLanguageState {
    language: RwLock<BackendLanguage>,
}

impl BackendLanguageState {
    pub fn get(&self) -> BackendLanguage {
        self.language
            .read()
            .map(|language| *language)
            .unwrap_or_default()
    }

    pub fn set(&self, language: BackendLanguage) {
        if let Ok(mut current) = self.language.write() {
            *current = language;
        }
    }
}

#[tauri::command]
pub fn backend_language_set(
    state: tauri::State<'_, BackendLanguageState>,
    language: String,
) -> Result<(), String> {
    state.set(BackendLanguage::from_app_language(&language));
    Ok(())
}

pub fn translate_gui_error(state: &BackendLanguageState, message: &str) -> String {
    match state.get() {
        BackendLanguage::Japanese => translate_to_japanese(message),
        BackendLanguage::English => message.to_string(),
    }
}

pub fn translate_for_app_language(language: &str, message: &str) -> String {
    match BackendLanguage::from_app_language(language) {
        BackendLanguage::Japanese => translate_to_japanese(message),
        BackendLanguage::English => message.to_string(),
    }
}

pub fn translate_api_error(message: &str) -> String {
    message.to_string()
}

fn translate_to_japanese(message: &str) -> String {
    if let Some(translation) = translate_special_case(message) {
        return translation;
    }
    if let Some(translation) = translate_prefixed(message) {
        return translation;
    }
    if let Some(translation) = translate_exact(message) {
        return translation.to_string();
    }
    message.to_string()
}

fn translate_special_case(message: &str) -> Option<String> {
    if let Some(rest) = message.strip_prefix("The specified serial port was not found: ") {
        if let Some((port, available)) = rest.split_once(". Available: ") {
            return Some(format!(
                "指定されたシリアルポートが見つかりません: {port}。利用可能: {available}"
            ));
        }
    }

    if let Some(rest) = message.strip_prefix("Commands must be no longer than ") {
        if let Some(limit) = rest.strip_suffix(" characters") {
            return Some(format!("コマンドは{}文字以内で指定してください", limit));
        }
    }

    if let Some(rest) = message.strip_prefix("Input must be no longer than ") {
        if let Some(limit) = rest.strip_suffix(" characters") {
            return Some(format!("入力は{}文字以内で指定してください", limit));
        }
    }

    if let Some(provider) = message
        .strip_suffix(" API key is not configured. Save the API key in Settings, then try again.")
    {
        return Some(format!(
            "{} の API キーが設定されていません。Settings で API キーを保存してから再試行してください。",
            provider
        ));
    }

    if let Some(rest) = message.strip_prefix("Could not parse the ") {
        if let Some(provider) = rest.strip_suffix(" model list response. Try again later.") {
            return Some(format!(
                "{} の model list 応答を解析できませんでした。しばらくしてから再試行してください。",
                provider
            ));
        }
        if let Some(provider) = rest.strip_suffix(" chat response. Try again later.") {
            return Some(format!(
                "{} の chat 応答を解析できませんでした。しばらくしてから再試行してください。",
                provider
            ));
        }
    }

    if let Some(details) = message.strip_prefix(
        "Could not connect to Ollama. Check that Ollama is running and the Base URL is correct. Details: ",
    ) {
        return Some(format!(
            "Ollama に接続できませんでした。Ollama が起動していること、Base URL が正しいことを確認してください。詳細: {}",
            details
        ));
    }

    if let Some(rest) = message.strip_prefix("Could not connect to ") {
        if let Some((provider, details)) = rest
            .split_once(". Check your network connection, proxy, and firewall settings. Details: ")
        {
            return Some(format!(
                "{} に接続できませんでした。ネットワーク接続、プロキシ、ファイアウォール設定を確認してください。詳細: {}",
                provider, details
            ));
        }
    }

    if let Some(rest) = message.strip_prefix("Could not read available models from the ") {
        if let Some((provider, details)) =
            rest.split_once(" response. The response format may have changed. Details: ")
        {
            return Some(format!(
                "{} の応答から利用可能なモデルを取得できませんでした。応答形式が変わった可能性があります。詳細: {}",
                provider, details
            ));
        }
    }

    if let Some(rest) = message.strip_prefix("Could not read the ") {
        if let Some((provider, details)) =
            rest.split_once(" chat response. Try again later. Details: ")
        {
            return Some(format!(
                "{} のチャット応答を読み取れませんでした。時間を置いて再試行してください。詳細: {}",
                provider, details
            ));
        }
    }

    if let Some(rest) = message.strip_prefix(
        "The SSH host key is untrusted. Verify the fingerprint before connecting: SHA256:",
    ) {
        return Some(format!(
            "SSHホスト鍵が未信頼です。接続前にフィンガープリントを確認してください: SHA256:{}",
            rest
        ));
    }

    if let Some(rest) = message
        .strip_prefix("The SSH host key does not match. A MITM attack may be in progress. Saved: ")
    {
        if let Some((saved, received)) = rest.split_once(" / Received: SHA256:") {
            return Some(format!(
                "SSHホスト鍵が一致しません。MITMの可能性があります。保存済み: {} / 受信: SHA256:{}",
                saved, received
            ));
        }
    }

    if let Some(rest) = message.strip_suffix(
        " authentication failed. Check that the API key is correct and has the required permissions.",
    ) {
        return Some(format!(
            "{} の認証に失敗しました。API キーが正しいこと、必要な権限があることを確認してください。",
            rest
        ));
    }

    if let Some(rest) = message.strip_suffix(
        " endpoint or model was not found. Check that the selected model is currently available.",
    ) {
        return Some(format!(
            "{} のエンドポイントまたはモデルが見つかりません。選択したモデルが現在利用可能か確認してください。",
            rest
        ));
    }

    if let Some(rest) = message.strip_suffix(
        " rate limit or quota was reached. Check your usage limits or try again later.",
    ) {
        return Some(format!(
            "{} のレート制限またはクォータに達しました。利用上限を確認するか、時間を置いて再試行してください。",
            rest
        ));
    }

    if let Some(rest) =
        message.strip_suffix(" is currently returning a server error. Try again later.")
    {
        return Some(format!(
            "{} 側で一時的な障害が発生しています。時間を置いて再試行してください。",
            rest
        ));
    }

    if let Some(rest) = message.strip_prefix("") {
        if let Some((provider, status)) = rest.split_once(" returned HTTP ") {
            if let Some(status) = status.strip_suffix(". Check your settings and selected model.") {
                return Some(format!(
                    "{} が HTTP {} を返しました。設定と選択したモデルを確認してください。",
                    provider, status
                ));
            }
        }
    }

    None
}

fn translate_prefixed(message: &str) -> Option<String> {
    for (en, ja) in [
        ("Failed to read log history: ", "ログ履歴読み込みエラー: "),
        ("Failed to parse log history: ", "ログ履歴解析エラー: "),
        (
            "Failed to create the log history directory: ",
            "ログ履歴ディレクトリ作成エラー: ",
        ),
        (
            "Failed to serialize log history: ",
            "ログ履歴シリアライズエラー: ",
        ),
        ("Failed to save log history: ", "ログ履歴保存エラー: "),
        (
            "Failed to verify the log directory: ",
            "ログディレクトリ確認エラー: ",
        ),
        (
            "Failed to verify the log file: ",
            "ログファイル確認エラー: ",
        ),
        (
            "Failed to delete the log file: ",
            "ログファイル削除エラー: ",
        ),
        ("Failed to create the log file: ", "ログ作成エラー: "),
        (
            "Failed to create the log directory: ",
            "ログディレクトリ作成エラー: ",
        ),
        ("Failed to write to the log file: ", "ログ書き込みエラー: "),
        ("Failed to create the window: ", "ウィンドウ作成エラー: "),
        ("Failed to list serial ports: ", "ポート一覧取得エラー: "),
        (
            "Failed to open the serial port: ",
            "シリアルポートオープンエラー: ",
        ),
        (
            "Failed to clone the serial port handle: ",
            "ポート複製エラー: ",
        ),
        ("Failed to send data: ", "送信エラー: "),
        ("Failed to connect over Telnet: ", "Telnet接続エラー: "),
        (
            "Failed to send the resize request: ",
            "リサイズ送信エラー: ",
        ),
        (
            "Failed to open the SSH channel: ",
            "SSHチャネルオープンエラー: ",
        ),
        (
            "Failed to open the SSH jump channel: ",
            "SSH踏み台チャネルオープンエラー: ",
        ),
        ("SSH connection error: ", "SSH接続エラー: "),
        (
            "SSH public key authentication error: ",
            "SSH公開鍵認証エラー: ",
        ),
        ("SSH authentication error: ", "SSH認証エラー: "),
        (
            "Failed to open the private key file: ",
            "秘密鍵ファイルを開けません: ",
        ),
        (
            "Failed to load the private key: ",
            "秘密鍵を読み込めません: ",
        ),
        (
            "Failed to send the external control credential prompt request: ",
            "外部制御の認証入力リクエスト送信エラー: ",
        ),
        (
            "Failed to send the external control log control request: ",
            "外部制御のログ制御リクエスト送信エラー: ",
        ),
        ("Failed to load the configuration: ", "設定読み込みエラー: "),
        ("Unknown connection type: ", "不明な接続種別: "),
    ] {
        if let Some(rest) = message.strip_prefix(en) {
            return Some(format!("{ja}{rest}"));
        }
    }

    None
}

fn translate_exact(message: &str) -> Option<&'static str> {
    match message {
        "Unknown log write mode: unknown" => Some("不明なログ書き込みモード: unknown"),
        "Unknown log mode: unknown" => Some("不明なログモード: unknown"),
        "Window not found" => Some("ウィンドウが見つかりません"),
        "The tab does not belong to this window" => Some("タブがこのウィンドウにありません"),
        "Source tab not found" => Some("移動元タブが見つかりません"),
        "Destination tab not found" => Some("移動先タブが見つかりません"),
        "Destination window not found" => Some("移動先ウィンドウが見つかりません"),
        "Source window not found" => Some("移動元ウィンドウが見つかりません"),
        "Tab not found" => Some("タブが見つかりません"),
        "The tab owner window does not match" => Some("タブの所有ウィンドウが一致しません"),
        "No tab is currently being dragged" => Some("ドラッグ中のタブがありません"),
        "Destination snapshot not found" => Some("移動先スナップショットが見つかりません"),
        "Session not found" => Some("セッションが見つかりません"),
        "The specified cursor is past the current output position" => {
            Some("指定されたカーソルは現在の出力位置より先です")
        }
        "The saved SSH host key does not match" => Some("保存済みのSSHホスト鍵と一致しません"),
        "The external control credential prompt request did not complete" => {
            Some("外部制御の認証入力リクエストが完了しませんでした")
        }
        "The external control credential prompt timed out" => {
            Some("外部制御の認証入力がタイムアウトしました")
        }
        "The external control credential prompt request was not found" => {
            Some("外部制御の認証入力リクエストが見つかりません")
        }
        "The external control credential prompt request has already finished" => {
            Some("外部制御の認証入力リクエストはすでに終了しています")
        }
        "The external control log control request did not complete" => {
            Some("外部制御のログ制御リクエストが完了しませんでした")
        }
        "The external control log control request timed out" => {
            Some("外部制御のログ制御リクエストがタイムアウトしました")
        }
        "The external control log control request was not found" => {
            Some("外部制御のログ制御リクエストが見つかりません")
        }
        "The external control log control request has already finished" => {
            Some("外部制御のログ制御リクエストはすでに終了しています")
        }
        "The session is already disconnected" => Some("セッションは切断済みです"),
        "Logger state required to start external control logging is unavailable" => {
            Some("外部制御ログ開始に必要なロガー状態がありません")
        }
        "Logger state required to stop external control logging is unavailable" => {
            Some("外部制御ログ停止に必要なロガー状態がありません")
        }
        "The command to send must not be empty" => Some("送信するコマンドが空です"),
        "App handle required to start external control logging is unavailable" => {
            Some("外部制御ログ開始に必要なアプリハンドルがありません")
        }
        "Log control state required to start external control logging is unavailable" => {
            Some("外部制御ログ開始に必要なログ制御状態がありません")
        }
        "The external control log start response did not include a log file path" => {
            Some("外部制御ログ開始応答にログファイルパスがありません")
        }
        "App handle required to stop external control logging is unavailable" => {
            Some("外部制御ログ停止に必要なアプリハンドルがありません")
        }
        "Log control state required to stop external control logging is unavailable" => {
            Some("外部制御ログ停止に必要なログ制御状態がありません")
        }
        "App handle required for external control connections is unavailable" => {
            Some("外部制御接続に必要なアプリハンドルがありません")
        }
        "Credential prompt state required for external control is unavailable" => {
            Some("外部制御の認証入力に必要な状態がありません")
        }
        "The external control credential prompt was cancelled" => {
            Some("外部制御の認証入力がキャンセルされました")
        }
        "New connections from external control are disabled. Set external_control.connect_enabled=true." => {
            Some(
                "外部制御からの新規接続は無効です。external_control.connect_enabled=true にしてください",
            )
        }
        "The SSH authentication method is invalid" => Some("SSH認証方式が不正です"),
        "The saved SSH profile does not have a private key file configured" => {
            Some("保存済みSSHプロファイルに秘密鍵ファイルが設定されていません")
        }
        "none" => Some("なし"),
        "data_bits must be 5, 6, 7, or 8" => {
            Some("data_bits は 5, 6, 7, 8 のいずれかを指定してください")
        }
        "stop_bits must be 1 or 2" => Some("stop_bits は 1 または 2 を指定してください"),
        "Specify port" => Some("port を指定してください"),
        "baud_rate must be at least 1" => Some("baud_rate は 1 以上で指定してください"),
        "Specify profile_id" => Some("profile_id を指定してください"),
        "This saved profile is disabled for external control" => {
            Some("この保存済みプロファイルは外部制御からの利用が無効です")
        }
        "New external control connections only support saved SSH and Telnet profiles" => {
            Some("外部制御の新規接続は保存済みSSH/Telnetプロファイルのみ対応しています")
        }
        "Saved profile not found" => Some("保存済みプロファイルが見つかりません"),
        "The saved profile does not have a host configured" => {
            Some("保存済みプロファイルにホストが設定されていません")
        }
        "The saved SSH profile does not have a username configured" => {
            Some("保存済みSSHプロファイルにユーザー名が設定されていません")
        }
        "An SSH jump profile cannot reference itself" => {
            Some("SSH踏み台プロファイルに自分自身は指定できません")
        }
        "SSH jump profile not found" => Some("SSH踏み台プロファイルが見つかりません"),
        "Only SSH profiles can be used as SSH jump profiles" => {
            Some("SSH踏み台にはSSHプロファイルのみ指定できます")
        }
        "Nested SSH jump profiles are not supported" => {
            Some("SSH踏み台の多段指定には対応していません")
        }
        "The SSH jump profile does not have a host configured" => {
            Some("SSH踏み台プロファイルにホストが設定されていません")
        }
        "The SSH jump profile does not have a username configured" => {
            Some("SSH踏み台プロファイルにユーザー名が設定されていません")
        }
        "The SSH jump profile does not have a private key file configured" => {
            Some("SSH踏み台プロファイルに秘密鍵ファイルが設定されていません")
        }
        "PTY request failed" => Some("PTYリクエストエラー"),
        "Shell request failed" => Some("シェルリクエストエラー"),
        "unknown" => Some("不明"),
        "SSH host key verification error" => Some("SSHホスト鍵検証エラー"),
        "SSH public key authentication error: specify a private key file" => {
            Some("SSH公開鍵認証エラー: 秘密鍵ファイルを指定してください")
        }
        "A public key file was specified instead of a private key file. Specify the private key itself." => {
            Some(
                "秘密鍵ファイルではなく公開鍵ファイルが指定されています。秘密鍵本体を指定してください",
            )
        }
        "PuTTY-format (.ppk) private keys cannot be loaded directly. Convert the key to OpenSSH format." => {
            Some(
                "PuTTY形式(.ppk)の秘密鍵は直接読み込めません。OpenSSH形式の秘密鍵に変換してください",
            )
        }
        "Specify an OpenSSH or PEM private key file. Public key files cannot be used as private keys." => {
            Some(
                "OpenSSH/PEM形式の秘密鍵ファイルを指定してください。公開鍵ファイルは秘密鍵として使用できません",
            )
        }
        "Specify a private key file" => Some("秘密鍵ファイルを指定してください"),
        "Failed to load the private key. Check the key format, passphrase, or file contents." => {
            Some(
                "秘密鍵を読み込めません。鍵形式、パスフレーズ、またはファイル内容を確認してください",
            )
        }
        "The private key is encrypted with a passphrase. Enter the key passphrase." => {
            Some("秘密鍵はパスフレーズで暗号化されています。鍵パスフレーズを入力してください")
        }
        "SSH authentication failed: the username or password is incorrect" => {
            Some("SSH認証失敗: ユーザー名またはパスワードが正しくありません")
        }
        "SSH public key authentication failed: check the username, private key, public key registration, or passphrase." => {
            Some(
                "SSH公開鍵認証失敗: ユーザー名、秘密鍵、公開鍵の登録状態、またはパスフレーズを確認してください",
            )
        }
        "Failed to retrieve the SSH host key" => Some("SSHホスト鍵を取得できませんでした"),
        "The most recently retrieved SSH host key was not found. Connect again." => {
            Some("直前に取得したSSHホスト鍵が見つかりません。もう一度接続してください。")
        }
        "SSH connection timed out" => Some("SSH接続がタイムアウトしました"),
        "SSH authentication timed out" => Some("SSH認証がタイムアウトしました"),
        "Opening the SSH channel timed out" => Some("SSHチャネルオープンがタイムアウトしました"),
        "Opening the SSH jump channel timed out" => {
            Some("SSH踏み台チャネルオープンがタイムアウトしました")
        }
        "PTY request timed out" => Some("PTYリクエストがタイムアウトしました"),
        "Shell request timed out" => Some("シェルリクエストがタイムアウトしました"),
        "SSH send error" => Some("SSH送信エラー"),
        "SSH send timed out" => Some("SSH送信がタイムアウトしました"),
        "SSH resize error" => Some("SSHリサイズエラー"),
        "SSH resize timed out" => Some("SSHリサイズがタイムアウトしました"),
        "No AI model is selected. Choose a model, then try again." => {
            Some("AI モデルが選択されていません。モデルを選択してから再試行してください。")
        }
        "Configure the Azure OpenAI endpoint and model deployment name in Settings." => {
            Some(
                "Azure OpenAI の Endpoint と Model deployment name を Settings で設定してください。",
            )
        }
        "The Azure OpenAI endpoint URL is invalid. Check the value in Settings." => {
            Some(
                "Azure OpenAI の Endpoint URL が正しくありません。Settings の入力内容を確認してください。",
            )
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_workspace_message_to_japanese() {
        assert_eq!(
            translate_for_app_language("ja", "Window not found"),
            "ウィンドウが見つかりません"
        );
    }

    #[test]
    fn leaves_english_message_as_is() {
        assert_eq!(
            translate_for_app_language("en", "Window not found"),
            "Window not found"
        );
    }
}
