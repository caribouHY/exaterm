import { useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  ForeignTabPlacement,
  WorkspaceDragDropResult,
  WorkspacePointerPosition,
} from "../../types";
import { planTabReorder } from "./tabStripModel";
import { workspaceClient } from "./workspaceClient";
import type { RemovalFocusContext, WindowTabsController } from "./useWindowTabs";

interface UseWorkspaceTabMovementOptions {
  tabs: WindowTabsController;
  flushLogBuffersForMove: (tabId: string) => Promise<void>;
}

export function useWorkspaceTabMovement({
  tabs,
  flushLogBuffersForMove,
}: UseWorkspaceTabMovementOptions) {
  const reorderTabs = useCallback(
    (draggedId: string, targetId: string, dropSide: "before" | "after") => {
      const currentAppTabs = tabs.getCurrentState().appTabs;
      const plan = planTabReorder(currentAppTabs, draggedId, targetId, dropSide);
      if (!plan) return;

      tabs.setVisibleTabOrder(plan.nextOrder);
      if (plan.persistToWorkspace) {
        workspaceClient
          .reorderTab(tabs.windowId, draggedId, targetId, dropSide)
          .then(tabs.applyWorkspaceSnapshot)
          .catch((error) => {
            console.error("Failed to reorder workspace tab:", error);
            tabs.rollbackVisibleTabOrder(plan.nextOrder, plan.previousOrder);
          });
      }
    },
    [tabs]
  );

  const startCrossWindowDrag = useCallback(
    (tabId: string, pointerScreenPosition: WorkspacePointerPosition) => {
      workspaceClient
        .startDrag(tabs.windowId, tabId, pointerScreenPosition)
        .then((preview) => {
          tabs.setDragPreview(preview.active ? preview : null);
        })
        .catch((error) => {
          console.error("Failed to start workspace tab drag:", error);
        });
    },
    [tabs]
  );

  const updateCrossWindowDrag = useCallback((pointerScreenPosition: WorkspacePointerPosition) => {
    workspaceClient.updateDrag(pointerScreenPosition).catch((error) => {
      console.error("Failed to update workspace tab drag:", error);
    });
  }, []);

  const hoverCrossWindowDrag = useCallback(
    (targetIndex: number | null, placement: ForeignTabPlacement | null) => {
      tabs.setForeignTabPlacement(placement);
      workspaceClient.hoverDrag(tabs.windowId, targetIndex).catch((error) => {
        console.error("Failed to update workspace tab drag hover:", error);
      });
    },
    [tabs]
  );

  const cancelCrossWindowDrag = useCallback(() => {
    workspaceClient
      .cancelDrag()
      .then(() => {
        tabs.setDragPreview(null);
      })
      .catch((error) => {
        console.error("Failed to cancel workspace tab drag:", error);
      });
  }, [tabs]);

  const completeMove = useCallback(
    async (result: WorkspaceDragDropResult, focusContext: RemovalFocusContext) => {
      result.snapshots.forEach(tabs.applyWorkspaceSnapshot);
      tabs.setDragPreview(null);
      const sourceSnapshot = result.snapshots.find(
        (snapshot) => snapshot.window_id === tabs.windowId
      );
      const movedFromCurrentWindow =
        result.source_window_id === tabs.windowId && result.target_window_id !== tabs.windowId;

      if (movedFromCurrentWindow) {
        tabs.restoreFocusAfterRemoval(focusContext);
      }
      if (
        movedFromCurrentWindow &&
        sourceSnapshot?.window.tab_order.length === 0 &&
        tabs.getCurrentState().utilityTabs.length === 0
      ) {
        await getCurrentWindow().close();
      }
    },
    [tabs]
  );

  const flushBeforeMove = useCallback(
    async (tabId: string) => {
      try {
        await flushLogBuffersForMove(tabId);
      } catch (error) {
        console.warn("Failed to flush terminal log buffers before tab move:", error);
      }
    },
    [flushLogBuffersForMove]
  );

  const dropCrossWindowDrag = useCallback(
    async (tabId: string, pointerScreenPosition: WorkspacePointerPosition) => {
      const focusContext = tabs.captureRemovalFocus(tabId);
      await flushBeforeMove(tabId);

      try {
        await completeMove(await workspaceClient.dropDrag(pointerScreenPosition), focusContext);
      } catch (error) {
        console.error("Failed to drop workspace tab drag:", error);
        cancelCrossWindowDrag();
      }
    },
    [cancelCrossWindowDrag, completeMove, flushBeforeMove, tabs]
  );

  const moveTabToNewWindow = useCallback(
    async (tabId: string) => {
      const focusContext = tabs.captureRemovalFocus(tabId);
      await flushBeforeMove(tabId);

      try {
        await completeMove(await workspaceClient.detachTab(tabId, tabs.windowId), focusContext);
      } catch (error) {
        console.error("Failed to move workspace tab to a new window:", error);
      }
    },
    [completeMove, flushBeforeMove, tabs]
  );

  return {
    reorderTabs,
    startCrossWindowDrag,
    updateCrossWindowDrag,
    hoverCrossWindowDrag,
    cancelCrossWindowDrag,
    dropCrossWindowDrag,
    moveTabToNewWindow,
  };
}
