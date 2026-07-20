import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  ConnectionType,
  Encoding,
  TerminalMode,
  WorkspaceConnectionInfo,
  WorkspaceDragDropResult,
  WorkspaceDragPreview,
  WorkspacePointerPosition,
  WorkspaceSnapshot,
  WorkspaceWindowCreateResult,
} from "../../types";

export interface WorkspaceTabMetadataPatch {
  title?: string;
  encoding?: Encoding;
  terminalMode?: TerminalMode;
  isConnected?: boolean;
  isAutoLogging?: boolean;
  isManualLogging?: boolean;
  isLoggingPaused?: boolean;
  manualLogFilePath?: string;
}

export interface RegisterWorkspaceTabInput {
  windowId: string;
  sessionId: string;
  connectionType: ConnectionType;
  title: string;
  encoding: Encoding;
  terminalMode: TerminalMode;
  connectionInfo?: WorkspaceConnectionInfo;
  isAutoLogging: boolean;
}

export const workspaceClient = {
  registerWindow(windowId: string) {
    return invoke<WorkspaceSnapshot>("workspace_window_register", {
      windowId,
      label: windowId,
      focused: true,
    });
  },

  focusWindow(windowId: string) {
    return invoke<WorkspaceSnapshot>("workspace_window_focus", { windowId });
  },

  createWindow() {
    return invoke<WorkspaceWindowCreateResult>("workspace_window_create");
  },

  registerTab(input: RegisterWorkspaceTabInput) {
    return invoke<WorkspaceSnapshot>("workspace_tab_register", {
      windowId: input.windowId,
      sessionId: input.sessionId,
      connectionType: input.connectionType,
      title: input.title,
      encoding: input.encoding,
      terminalMode: input.terminalMode,
      connectionInfo: input.connectionInfo,
      isAutoLogging: input.isAutoLogging,
    });
  },

  activateTab(windowId: string, tabId: string) {
    return invoke<WorkspaceSnapshot>("workspace_tab_activate", { windowId, tabId });
  },

  removeTab(windowId: string, tabId: string) {
    return invoke<WorkspaceSnapshot>("workspace_tab_remove", { windowId, tabId });
  },

  reorderTab(
    windowId: string,
    draggedTabId: string,
    targetTabId: string,
    dropSide: "before" | "after"
  ) {
    return invoke<WorkspaceSnapshot>("workspace_tab_reorder", {
      windowId,
      draggedTabId,
      targetTabId,
      dropSide,
    });
  },

  updateTabMetadata(tabId: string, patch: WorkspaceTabMetadataPatch) {
    return invoke<WorkspaceSnapshot>("workspace_tab_update_metadata", { tabId, patch });
  },

  startDrag(windowId: string, tabId: string, pointerScreenPosition: WorkspacePointerPosition) {
    return invoke<WorkspaceDragPreview>("workspace_tab_drag_start", {
      windowId,
      tabId,
      pointerScreenPosition,
    });
  },

  updateDrag(pointerScreenPosition: WorkspacePointerPosition) {
    return invoke<WorkspaceDragPreview>("workspace_tab_drag_update", {
      pointerScreenPosition,
    });
  },

  hoverDrag(windowId: string, targetIndex: number | null) {
    return invoke<WorkspaceDragPreview>("workspace_tab_drag_hover", {
      windowId,
      targetIndex,
    });
  },

  dropDrag(pointerScreenPosition: WorkspacePointerPosition) {
    return invoke<WorkspaceDragDropResult>("workspace_tab_drag_drop", {
      pointerScreenPosition,
    });
  },

  cancelDrag() {
    return invoke<WorkspaceDragPreview>("workspace_tab_drag_cancel");
  },

  detachTab(tabId: string, fromWindowId: string) {
    return invoke<WorkspaceDragDropResult>("workspace_tab_detach_to_new_window", {
      tabId,
      fromWindowId,
    });
  },

  listenWorkspaceUpdated(handler: (snapshot: WorkspaceSnapshot) => void) {
    return listen<WorkspaceSnapshot>("workspace://updated", (event) => {
      handler(event.payload);
    });
  },

  listenDragPreview(handler: (preview: WorkspaceDragPreview) => void) {
    return listen<WorkspaceDragPreview>("workspace://drag-preview", (event) => {
      handler(event.payload);
    });
  },
};
