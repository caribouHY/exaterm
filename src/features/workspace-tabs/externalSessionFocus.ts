import type { TabInfo, WorkspaceSnapshot } from "../../types";

export interface SessionFocusRequest {
  request_id: string;
  session_id: string;
  tab_id: string;
  snapshot: WorkspaceSnapshot;
}

interface FocusResponse {
  requestId: string;
  tabId: string;
  selected: boolean;
}

export interface SessionFocusRenderedState {
  activeTabId: string | null;
  tabs: TabInfo[];
  closingTabIds: string[];
}

export function createExternalSessionFocusController(dependencies: {
  select(request: SessionFocusRequest): boolean;
  submit(response: FocusResponse): Promise<void>;
  focusTerminal(tabId: string): void;
  onError(): void;
}) {
  const pending = new Map<
    string,
    { request: SessionFocusRequest; accepted: boolean; version: number }
  >();
  let disposed = false;
  const submit = (request: SessionFocusRequest, selected: boolean) => {
    void dependencies
      .submit({ requestId: request.request_id, tabId: request.tab_id, selected })
      .catch(dependencies.onError);
  };
  return {
    receive(request: SessionFocusRequest, version: number) {
      if (disposed || pending.has(request.request_id)) return;
      pending.set(request.request_id, { request, accepted: dependencies.select(request), version });
    },
    afterCommit(state: SessionFocusRenderedState, committedVersion: number) {
      for (const { request, accepted, version } of pending.values()) {
        if (version > committedVersion) continue;
        pending.delete(request.request_id);
        const selected =
          accepted &&
          state.activeTabId === request.tab_id &&
          !state.closingTabIds.includes(request.tab_id) &&
          state.tabs.some(
            (tab) => tab.id === request.tab_id && tab.sessionId === request.session_id
          );
        if (selected) dependencies.focusTerminal(request.tab_id);
        submit(request, selected);
      }
    },
    dispose() {
      disposed = true;
      for (const { request } of pending.values()) submit(request, false);
      pending.clear();
    },
  };
}
