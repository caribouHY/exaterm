use std::time::Duration;
use tokio::time::{self, Instant};
use uuid::Uuid;

use super::{
    invalid_params, not_found, unavailable, ExternalControlError,
    ExternalControlFocusRequestPayload, ExternalControlService, FocusTerminalSessionArgs,
    FocusTerminalSessionResult,
};

impl ExternalControlService {
    pub(crate) async fn focus_terminal_session(
        &self,
        args: FocusTerminalSessionArgs,
    ) -> Result<FocusTerminalSessionResult, ExternalControlError> {
        if args.session_id.trim().is_empty() {
            return Err(invalid_params("Session ID must not be empty"));
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        for attempt in 0..2 {
            let snapshot = self
                .runtime
                .workspace
                .activate_session(&args.session_id)
                .await
                .ok_or_else(|| not_found("Session tab not found"))?;
            let window_id = snapshot.window_id.clone();
            let tab_id = snapshot
                .window
                .active_tab_id
                .clone()
                .ok_or_else(|| unavailable("Session tab is unavailable"))?;
            self.runtime.io.ui.emit_workspace_updated(&snapshot);
            let selected = time::timeout_at(
                deadline,
                self.runtime
                    .io
                    .ui
                    .request_session_focus(ExternalControlFocusRequestPayload {
                        request_id: Uuid::new_v4().to_string(),
                        session_id: args.session_id.clone(),
                        tab_id: tab_id.clone(),
                        snapshot,
                    }),
            )
            .await
            .map_err(|_| unavailable("Session focus request timed out"))?
            .map_err(unavailable);
            let current = self
                .runtime
                .workspace
                .tab_for_session(&args.session_id)
                .await
                .ok_or_else(|| not_found("Session tab not found"))?;
            if current.owner_window_id != window_id || current.tab_id != tab_id {
                if attempt == 0 && Instant::now() < deadline {
                    continue;
                }
                return Err(unavailable("Session tab moved during focus"));
            }
            if !selected? {
                return Err(unavailable("The GUI did not select the session tab"));
            }
            let focused = self
                .runtime
                .workspace
                .focus_session_window(&args.session_id, &window_id, &tab_id, || {
                    self.runtime.io.ui.focus_window(&window_id)
                })
                .await?;
            if focused {
                return Ok(FocusTerminalSessionResult {
                    session_id: args.session_id,
                    window_id,
                    tab_id,
                    focused: true,
                });
            }
            let current = self
                .runtime
                .workspace
                .tab_for_session(&args.session_id)
                .await
                .ok_or_else(|| not_found("Session tab not found"))?;
            if attempt == 0 && current.owner_window_id != window_id && Instant::now() < deadline {
                continue;
            }
            return Err(unavailable("Session tab changed during focus"));
        }
        Err(unavailable("Session tab moved during focus"))
    }
}
