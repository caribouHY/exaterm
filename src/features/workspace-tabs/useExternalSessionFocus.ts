import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { WindowTabsController } from "./useWindowTabs";
import {
  createExternalSessionFocusController,
  type SessionFocusRequest,
} from "./externalSessionFocus";

export function useExternalSessionFocus(
  windowTabs: WindowTabsController,
  focusTerminal: (tabId: string) => void
) {
  const { selectSessionForExternalFocus, windowId } = windowTabs;
  const focusTerminalRef = useRef(focusTerminal);
  focusTerminalRef.current = focusTerminal;
  const controllerRef = useRef<ReturnType<typeof createExternalSessionFocusController> | null>(
    null
  );
  const [requestVersion, setRequestVersion] = useState(0);
  const requestVersionRef = useRef(0);

  useEffect(() => {
    const controller = createExternalSessionFocusController({
      select: (request) =>
        request.snapshot.window_id === windowId &&
        selectSessionForExternalFocus(request.snapshot, request.session_id, request.tab_id),
      submit: (response) => invoke("external_control_session_focus_submit", { ...response }),
      focusTerminal: (tabId) => {
        focusTerminalRef.current(tabId);
      },
      onError: () => {
        console.error("Failed to acknowledge a session focus request.");
      },
    });
    controllerRef.current = controller;
    let disposed = false;
    const unlisten = listen<SessionFocusRequest>(
      "external-control://session-focus-request",
      (event) => {
        if (disposed) return;
        const version = ++requestVersionRef.current;
        controller.receive(event.payload, version);
        setRequestVersion(version);
      }
    );
    void unlisten.catch(() => {
      console.error("Failed to register the session focus listener.");
    });
    return () => {
      disposed = true;
      controller.dispose();
      if (controllerRef.current === controller) controllerRef.current = null;
      void unlisten.then(
        (stop) => stop(),
        () => {}
      );
    };
  }, [selectSessionForExternalFocus, windowId]);

  useEffect(() => {
    controllerRef.current?.afterCommit(
      {
        activeTabId: windowTabs.activeTabId,
        tabs: windowTabs.tabs,
        closingTabIds: windowTabs.closingTabIds,
      },
      requestVersion
    );
  }, [requestVersion, windowTabs.activeTabId, windowTabs.tabs, windowTabs.closingTabIds]);
}
