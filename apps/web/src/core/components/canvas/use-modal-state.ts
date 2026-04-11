"use client";

import { useCallback, useState, type Dispatch, type SetStateAction } from "react";

interface ConfirmAction {
  title: string;
  message: string;
  confirmLabel: string;
  variant: "danger" | "default";
  onConfirm: () => void;
}

interface CommentDialogState {
  nodeId: string;
  initialValue: string;
}

interface UseModalStateReturn {
  // Config modal
  configModalNodeId: string | null;
  setConfigModalNodeId: (id: string | null) => void;

  // Shortcut help
  showShortcuts: boolean;
  setShowShortcuts: Dispatch<SetStateAction<boolean>>;
  handleCloseShortcuts: () => void;

  // Confirm dialog
  confirmAction: ConfirmAction | null;
  setConfirmAction: (action: ConfirmAction | null) => void;

  // Comment dialog
  commentDialog: CommentDialogState | null;
  setCommentDialog: (dialog: CommentDialogState | null) => void;

  // Context menu
  contextMenu: { x: number; y: number; nodeId?: string; edgeId?: string } | null;
  setContextMenu: (menu: { x: number; y: number; nodeId?: string; edgeId?: string } | null) => void;

  // Canvas search
  showSearch: boolean;
  setShowSearch: Dispatch<SetStateAction<boolean>>;

  // Snap to grid
  snapToGrid: boolean;
  setSnapToGrid: Dispatch<SetStateAction<boolean>>;
}

export function useModalState(): UseModalStateReturn {
  // Config modal
  const [configModalNodeId, setConfigModalNodeId] = useState<string | null>(null);

  // Shortcut help
  const [showShortcuts, setShowShortcuts] = useState(false);
  const handleCloseShortcuts = useCallback(() => setShowShortcuts(false), []);

  // Confirm dialog
  const [confirmAction, setConfirmAction] = useState<ConfirmAction | null>(null);

  // Comment dialog
  const [commentDialog, setCommentDialog] = useState<CommentDialogState | null>(null);

  // Context menu
  const [contextMenu, setContextMenu] = useState<{
    x: number; y: number; nodeId?: string; edgeId?: string;
  } | null>(null);

  // Canvas search
  const [showSearch, setShowSearch] = useState(false);

  // Snap to grid
  const [snapToGrid, setSnapToGrid] = useState(false);

  return {
    configModalNodeId,
    setConfigModalNodeId,
    showShortcuts,
    setShowShortcuts,
    handleCloseShortcuts,
    confirmAction,
    setConfirmAction,
    commentDialog,
    setCommentDialog,
    contextMenu,
    setContextMenu,
    showSearch,
    setShowSearch,
    snapToGrid,
    setSnapToGrid,
  };
}
