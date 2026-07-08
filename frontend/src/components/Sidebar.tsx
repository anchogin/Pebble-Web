import { useEffect, useMemo, useRef, useState } from "react";
import {
  Inbox,
  Send,
  FileEdit,
  Trash2,
  Archive,
  AlertTriangle,
  Folder,
  LayoutGrid,
  Settings,
  Search,
  Clock,
  Star,
  ChevronRight,
  ChevronDown,
  Plus,
  Pencil,
  Link,
  Link2Off,
  MoreHorizontal,
} from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { useUIStore } from "../stores/ui.store";
import { isComposeDirty, useComposeStore } from "../stores/compose.store";
import { useConfirmStore } from "../stores/confirm.store";
import { useMailStore } from "../stores/mail.store";
import { useAccountsQuery, useFoldersForAccountsQuery } from "../hooks/queries";
import { useFolderUnreadCountsForAccounts } from "../hooks/queries/useFolderUnreadCounts";
import {
  ALL_ACCOUNTS_SELECT_VALUE,
  buildAllAccountsFolders,
  buildFolderTree,
  sortFoldersForSidebar,
  unreadCountForFolder,
  type FolderTreeNode,
} from "../lib/folderAggregation";
import type { Account, Folder as FolderType } from "../lib/api";
import { createFolder, deleteFolder, linkFolder, renameFolder, unlinkFolder } from "../lib/api";

const EMPTY_ACCOUNTS: Account[] = [];
const EMPTY_FOLDERS: FolderType[] = [];

type FolderNameDialogState =
  | { mode: "create"; value: string }
  | { mode: "rename"; folder: FolderType; value: string };

const ROLE_ICONS: Record<string, React.ReactNode> = {
  inbox: <Inbox size={16} />,
  sent: <Send size={16} />,
  drafts: <FileEdit size={16} />,
  trash: <Trash2 size={16} />,
  archive: <Archive size={16} />,
  spam: <AlertTriangle size={16} />,
};

function folderIcon(role: FolderType["role"]): React.ReactNode {
  return (role && ROLE_ICONS[role]) || <Folder size={16} />;
}

// Default folders shown when no account is configured
const DEFAULT_FOLDERS: { role: string; labelKey: string }[] = [
  { role: "inbox", labelKey: "sidebar.inbox" },
  { role: "sent", labelKey: "sidebar.sent" },
  { role: "archive", labelKey: "sidebar.archive" },
  { role: "drafts", labelKey: "sidebar.drafts" },
  { role: "trash", labelKey: "sidebar.trash" },
  { role: "spam", labelKey: "sidebar.spam" },
];

export default function Sidebar() {
  const { t } = useTranslation();
  const activeView = useUIStore((s) => s.activeView);
  const setActiveView = useUIStore((s) => s.setActiveView);
  const sidebarCollapsed = useUIStore((s) => s.sidebarCollapsed);
  const activeFolderId = useMailStore((s) => s.activeFolderId);
  const activeAccountId = useMailStore((s) => s.activeAccountId);
  const setActiveAccountId = useMailStore((s) => s.setActiveAccountId);
  const setActiveFolderId = useMailStore((s) => s.setActiveFolderId);
  const queryClient = useQueryClient();

  const showUnread = useUIStore((s) => s.showFolderUnreadCount);
  const { data: accounts = EMPTY_ACCOUNTS } = useAccountsQuery();
  const hasAccounts = accounts.length > 0;
  const allAccountsMode = accounts.length > 1 && !activeAccountId;
  const folderAccountIds = useMemo(
    () => activeAccountId ? [activeAccountId] : accounts.map((account) => account.id),
    [accounts, activeAccountId],
  );
  const { data: folders = EMPTY_FOLDERS, isFetched: foldersFetched } = useFoldersForAccountsQuery(folderAccountIds);
  const { data: unreadCounts = {} } = useFolderUnreadCountsForAccounts(folderAccountIds);
  const ROLE_LABELS: Record<string, string> = {
    inbox: t("sidebar.inbox"),
    sent: t("sidebar.sent"),
    drafts: t("sidebar.drafts"),
    trash: t("sidebar.trash"),
    archive: t("sidebar.archive"),
    spam: t("sidebar.spam"),
  };
  const folderLabel = (folder: FolderType) => (folder.role && ROLE_LABELS[folder.role]) || folder.name;

  const displayedFolders = useMemo(
    () => allAccountsMode ? buildAllAccountsFolders(folders) : folders,
    [allAccountsMode, folders],
  );
  const hasRealFolders = displayedFolders.length > 0;

  // Keep system folders stable across all-account and single-account views.
  const dedupedFolders = useMemo(() => {
    return sortFoldersForSidebar(displayedFolders);
  }, [displayedFolders]);

  const systemFolders = useMemo(
    () => dedupedFolders.filter((f) => f.role),
    [dedupedFolders],
  );

  const customFolderTree = useMemo(
    () => buildFolderTree(dedupedFolders.filter((f) => !f.role)),
    [dedupedFolders],
  );

  const [collapsedPaths, setCollapsedPaths] = useState<Set<string>>(new Set());
  const [folderActionId, setFolderActionId] = useState<string | null>(null);
  const [openFolderMenuId, setOpenFolderMenuId] = useState<string | null>(null);
  const [folderNameDialog, setFolderNameDialog] = useState<FolderNameDialogState | null>(null);

  const toggleCollapsed = (path: string) => {
    setCollapsedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  useEffect(() => {
    if (!openFolderMenuId) return;

    function handleDocumentClick() {
      setOpenFolderMenuId(null);
    }

    function handleDocumentKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setOpenFolderMenuId(null);
      }
    }

    document.addEventListener("click", handleDocumentClick);
    document.addEventListener("keydown", handleDocumentKeyDown);

    return () => {
      document.removeEventListener("click", handleDocumentClick);
      document.removeEventListener("keydown", handleDocumentKeyDown);
    };
  }, [openFolderMenuId]);

  // Auto-select the only account. With multiple accounts, null means the
  // combined "all accounts" mailbox.
  useEffect(() => {
    if (accounts.length === 1 && !activeAccountId) {
      setActiveAccountId(accounts[0].id);
    }
  }, [accounts, activeAccountId, setActiveAccountId]);

  // Auto-select inbox folder when folders load.
  // If the selected account has no folders, try the next account.
  useEffect(() => {
    if (displayedFolders.length > 0 && !activeFolderId) {
      const inbox = displayedFolders.find((f) => f.role === "inbox");
      setActiveFolderId((inbox ?? displayedFolders[0]).id);
    } else if (!allAccountsMode && foldersFetched && displayedFolders.length === 0 && activeAccountId && accounts.length > 1) {
      const activeAccount = accounts.find((a) => a.id === activeAccountId);
      if (activeAccount && activeAccount.provider !== "gmail") {
        const idx = accounts.findIndex((a) => a.id === activeAccountId);
        const next = accounts[idx + 1] ?? accounts.find((a) => a.id !== activeAccountId);
        if (next) {
          setActiveAccountId(next.id);
        }
      }
    }
  }, [displayedFolders, foldersFetched, activeFolderId, setActiveFolderId, accounts, activeAccountId, setActiveAccountId, allAccountsMode]);

  async function confirmDiscardDraft() {
    if (isComposeDirty()) {
      const confirmed = await useConfirmStore.getState().confirm({
        title: t("compose.discardDraft", "Discard draft"),
        message: t("compose.discardDraftConfirm", "You have an unsaved draft. Discard and leave?"),
        destructive: true,
      });
      return confirmed;
    }
    return true;
  }

  async function safeSetActiveView(view: Parameters<typeof setActiveView>[0]) {
    if (isComposeDirty()) {
      const confirmed = await confirmDiscardDraft();
      if (!confirmed) return;
      useComposeStore.getState().discardComposeAndSetActiveView(view);
      return;
    }
    setActiveView(view);
  }

  async function handleFolderClick(folderId: string) {
    if (isComposeDirty()) {
      const confirmed = await confirmDiscardDraft();
      if (!confirmed) return;
      useComposeStore.getState().discardComposeAndSetActiveView("inbox");
      setActiveFolderId(folderId);
      return;
    }
    setActiveView("inbox");
    setActiveFolderId(folderId);
  }

  async function handleDefaultFolderClick() {
    await safeSetActiveView(hasAccounts ? "inbox" : "settings");
  }

  async function refreshFolders(accountId?: string) {
    if (accountId) {
      await queryClient.invalidateQueries({ queryKey: ["folders", accountId] });
    }
    await queryClient.invalidateQueries({ queryKey: ["folders"] });
  }

  async function runFolderAction(folderId: string, action: () => Promise<void>) {
    setFolderActionId(folderId);
    try {
      await action();
    } finally {
      setFolderActionId(null);
    }
  }

  async function handleCreateFolder() {
    if (!activeAccountId) {
      await useConfirmStore.getState().confirm({
        title: t("sidebar.createFolder", "Create folder"),
        message: t("sidebar.selectAccountBeforeCreateFolder", "Select an account before creating a folder."),
        confirmLabel: t("common.confirm", "Confirm"),
      });
      return;
    }
    setFolderNameDialog({ mode: "create", value: "" });
  }

  function handleRenameFolder(folder: FolderType) {
    setFolderNameDialog({ mode: "rename", folder, value: folder.name });
  }

  async function handleFolderNameDialogSubmit() {
    if (!folderNameDialog) return;

    const trimmed = folderNameDialog.value.trim();
    if (!trimmed) return;

    if (folderNameDialog.mode === "rename") {
      if (trimmed === folderNameDialog.folder.name) return;

      await runFolderAction(folderNameDialog.folder.id, async () => {
        await renameFolder(folderNameDialog.folder.id, trimmed);
        await refreshFolders(folderNameDialog.folder.account_id);
      });
      setFolderNameDialog(null);
      return;
    }

    if (!activeAccountId) {
      await useConfirmStore.getState().confirm({
        title: t("sidebar.createFolder", "Create folder"),
        message: t("sidebar.selectAccountBeforeCreateFolder", "Select an account before creating a folder."),
        confirmLabel: t("common.confirm", "Confirm"),
      });
      setFolderNameDialog(null);
      return;
    }

    setFolderActionId("__create__");
    try {
      const folder = await createFolder(activeAccountId, trimmed);
      await refreshFolders(activeAccountId);
      setActiveView("inbox");
      setActiveFolderId(folder.id);
      setFolderNameDialog(null);
    } finally {
      setFolderActionId(null);
    }
  }

  async function handleDeleteFolder(folder: FolderType) {
    const confirmed = await useConfirmStore.getState().confirm({
      title: t("sidebar.deleteFolder", "Delete folder"),
      message: t("sidebar.deleteFolderConfirm", "Delete {{name}}? Messages stay in their mailboxes.", { name: folder.name }),
      destructive: true,
      confirmLabel: t("common.delete", "Delete"),
    });
    if (!confirmed) return;

    await runFolderAction(folder.id, async () => {
      await deleteFolder(folder.id);
      if (activeFolderId === folder.id) {
        setActiveFolderId(null);
      }
      await refreshFolders(folder.account_id);
    });
  }

  async function handleLinkFolder(folder: FolderType) {
    await runFolderAction(folder.id, async () => {
      await linkFolder(folder.id);
      await refreshFolders(folder.account_id);
    });
  }

  async function handleUnlinkFolder(folder: FolderType) {
    await runFolderAction(folder.id, async () => {
      await unlinkFolder(folder.id);
      await refreshFolders(folder.account_id);
    });
  }

  const buttonBase: React.CSSProperties = {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    borderRadius: "6px",
    padding: sidebarCollapsed ? "7px" : "6px 10px",
    width: "100%",
    border: "none",
    cursor: "pointer",
    fontSize: "13px",
    textAlign: "left",
    justifyContent: sidebarCollapsed ? "center" : "flex-start",
  };

  const folderNameValue = folderNameDialog?.value ?? "";
  const folderNameTrimmed = folderNameValue.trim();
  const folderNameSubmitDisabled =
    !folderNameDialog
    || !folderNameTrimmed
    || folderActionId !== null
    || (folderNameDialog.mode === "rename" && folderNameTrimmed === folderNameDialog.folder.name);

  return (
    <>
    <aside
      aria-label={t("sidebar.navigation", "Sidebar")}
      style={{
        width: sidebarCollapsed ? "48px" : "200px",
        flexShrink: 0,
        backgroundColor: "var(--color-sidebar-bg)",
        transition: "width 150ms ease",
        display: "flex",
        flexDirection: "column",
        height: "100%",
        overflow: "hidden",
        position: "relative",
        zIndex: 2,
        pointerEvents: "auto",
      }}
    >
      {/* Search button */}
      <nav aria-label={t("sidebar.search", "Search")} style={{ padding: "8px 6px 0", display: "flex", flexDirection: "column", gap: "1px" }}>
        <SidebarButton
          icon={<Search size={16} />}
          label={t("search.title", "Search")}
          isActive={activeView === "search"}
          collapsed={sidebarCollapsed}
          style={buttonBase}
          onClick={() => void safeSetActiveView("search")}
        />
      </nav>

      {/* Section label */}
      {!sidebarCollapsed && (
        <div style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "12px 8px 4px 10px",
          gap: "8px",
        }}>
          <span style={{
            fontSize: "11px",
            fontWeight: 600,
            color: "var(--color-text-secondary)",
            textTransform: "uppercase",
            letterSpacing: "0.5px",
          }}>
            {t("sidebar.mail", "Mail")}
          </span>
          {hasAccounts && (
            <button
              type="button"
              aria-label={t("sidebar.createFolder", "Create folder")}
              title={t("sidebar.createFolder", "Create folder")}
              disabled={folderActionId === "__create__"}
              onClick={() => void handleCreateFolder()}
              style={{
                width: "22px",
                height: "22px",
                borderRadius: "6px",
                border: "none",
                backgroundColor: "transparent",
                color: "var(--color-text-secondary)",
                cursor: folderActionId === "__create__" ? "default" : "pointer",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                opacity: folderActionId === "__create__" ? 0.45 : 1,
              }}
            >
              <Plus size={14} />
            </button>
          )}
        </div>
      )}

      {/* Folders section */}
      <nav
        className="scroll-region sidebar-folder-scroll"
        aria-label={t("sidebar.mailFolders", "Mail folders")}
        style={{
          flex: 1,
          overflowY: "auto",
          padding: "0 6px",
          display: "flex",
          flexDirection: "column",
          gap: "1px",
        }}
      >
        {hasRealFolders
          ? [
              ...systemFolders.flatMap((folder) => {
                const items: React.ReactNode[] = [];
                if (folder.role === "drafts") {
                  items.push(
                    <SidebarButton
                      key="__starred__"
                      icon={<Star size={16} />}
                      label={t("sidebar.starred", "Starred")}
                      isActive={activeView === "starred"}
                      collapsed={sidebarCollapsed}
                      style={buttonBase}
                      onClick={() => void safeSetActiveView("starred")}
                    />
                  );
                }
                const isActive = folder.id === activeFolderId && activeView === "inbox";
                items.push(
                  <SidebarButton
                    key={folder.id}
                    icon={folderIcon(folder.role)}
                    label={folderLabel(folder)}
                    badge={showUnread ? unreadCountForFolder(folder.id, folders, unreadCounts) : undefined}
                    isActive={isActive}
                    collapsed={sidebarCollapsed}
                    style={buttonBase}
                    onClick={() => void handleFolderClick(folder.id)}
                  />
                );
                return items;
              }),
              ...customFolderTree.map((node) => (
                <FolderTreeItem
                  key={node.folder?.id ?? `virt:${node.label}`}
                  node={node}
                  depth={0}
                  path={node.label}
                  activeFolderId={activeFolderId}
                  activeView={activeView}
                  collapsed={sidebarCollapsed}
                  collapsedPaths={collapsedPaths}
                  onToggleCollapse={toggleCollapsed}
                  buttonBase={buttonBase}
                  showUnread={showUnread}
                  folders={folders}
                  unreadCounts={unreadCounts}
                  onFolderClick={(id) => void handleFolderClick(id)}
                  onRenameFolder={(folder) => void handleRenameFolder(folder)}
                  onDeleteFolder={(folder) => void handleDeleteFolder(folder)}
                  onLinkFolder={(folder) => void handleLinkFolder(folder)}
                  onUnlinkFolder={(folder) => void handleUnlinkFolder(folder)}
                  actionFolderId={folderActionId}
                  openFolderMenuId={openFolderMenuId}
                  onOpenFolderMenuChange={setOpenFolderMenuId}
                  allowFolderActions
                />
              )),
            ]
          : DEFAULT_FOLDERS.flatMap((df, index) => {
              const items: React.ReactNode[] = [];
              if (df.role === "drafts") {
                items.push(
                  <SidebarButton
                    key="__starred__"
                    icon={<Star size={16} />}
                    label={t("sidebar.starred", "Starred")}
                    isActive={activeView === "starred"}
                    collapsed={sidebarCollapsed}
                    style={buttonBase}
                    onClick={() => void safeSetActiveView("starred")}
                  />
                );
              }
              items.push(
                <SidebarButton
                  key={df.role}
                  icon={ROLE_ICONS[df.role] || <Folder size={16} />}
                  label={t(df.labelKey)}
                  isActive={index === 0 && activeView === "inbox"}
                  collapsed={sidebarCollapsed}
                  style={buttonBase}
                  onClick={() => void handleDefaultFolderClick()}
                />
              );
              return items;
            })}
      </nav>

      {/* Account switcher — above divider */}
      {accounts.length > 1 && (
        <div style={{ padding: sidebarCollapsed ? "6px" : "6px 8px" }}>
          {sidebarCollapsed ? (
            <button
              type="button"
              title={activeAccountId
                ? (accounts.find((a) => a.id === activeAccountId)?.email ?? "")
                : t("sidebar.allAccounts", "All accounts")}
              onClick={() => {
                const idx = activeAccountId
                  ? accounts.findIndex((a) => a.id === activeAccountId)
                  : -1;
                const next = accounts[(idx + 1) % (accounts.length + 1)];
                setActiveAccountId(next ? next.id : null);
                setActiveFolderId(null);
              }}
              style={{
                width: "100%",
                height: "32px",
                borderRadius: "6px",
                border: "1.5px solid var(--color-border)",
                backgroundColor: "var(--color-bg)",
                color: "var(--color-accent)",
                fontSize: "11px",
                fontWeight: 700,
                cursor: "pointer",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              {activeAccountId
                ? (accounts.find((a) => a.id === activeAccountId)?.email?.[0] ?? "?").toUpperCase()
                : "✦"}
            </button>
          ) : (
            <select
              aria-label={t("settings.emailAccounts", "Email Accounts")}
              value={activeAccountId ?? ALL_ACCOUNTS_SELECT_VALUE}
              onChange={(e) => {
                setActiveAccountId(e.target.value === ALL_ACCOUNTS_SELECT_VALUE ? null : e.target.value);
                setActiveFolderId(null);
              }}
              style={{
                width: "100%",
                padding: "6px 8px",
                fontSize: "12px",
                borderRadius: "6px",
                border: "1.5px solid var(--color-border)",
                backgroundColor: "var(--color-bg)",
                color: "var(--color-text-primary)",
                cursor: "pointer",
              }}
            >
              <option value={ALL_ACCOUNTS_SELECT_VALUE}>
                {t("sidebar.allAccounts", "All accounts")}
              </option>
              {accounts.map((acc) => (
                <option key={acc.id} value={acc.id}>
                  {acc.email}
                </option>
              ))}
            </select>
          )}
        </div>
      )}

      {/* Divider */}
      <div
        style={{
          height: "1px",
          backgroundColor: "var(--color-border)",
          margin: "0 6px",
        }}
      />

      {/* Bottom nav: Snoozed + Kanban + Settings */}
      <nav
        aria-label={t("sidebar.tools", "Tools")}
        style={{
          padding: "6px 6px 8px",
          display: "flex",
          flexDirection: "column",
          gap: "1px",
        }}
      >
        <SidebarButton
          icon={<Clock size={16} />}
          label={t("sidebar.snoozed", "Snoozed")}
          isActive={activeView === "snoozed"}
          collapsed={sidebarCollapsed}
          style={buttonBase}
          onClick={() => void safeSetActiveView("snoozed")}
        />
        <SidebarButton
          icon={<LayoutGrid size={16} />}
          label={t("sidebar.kanban", "Kanban")}
          isActive={activeView === "kanban"}
          collapsed={sidebarCollapsed}
          style={buttonBase}
          onClick={() => void safeSetActiveView("kanban")}
        />
        <SidebarButton
          icon={<Settings size={16} />}
          label={t("sidebar.settings", "Settings")}
          isActive={activeView === "settings"}
          collapsed={sidebarCollapsed}
          style={buttonBase}
          onClick={() => void safeSetActiveView("settings")}
        />
      </nav>
    </aside>
    {folderNameDialog && (
      <FolderNameDialog
        title={folderNameDialog.mode === "create"
          ? t("sidebar.createFolder", "Create folder")
          : t("sidebar.renameFolderPrompt", "Rename folder")}
        inputLabel={t("sidebar.createFolderPrompt", "Folder name")}
        value={folderNameValue}
        confirmLabel={folderNameDialog.mode === "create"
          ? t("common.create", "Create")
          : t("sidebar.renameFolderAction", "Rename")}
        disabled={folderNameSubmitDisabled}
        pending={folderActionId !== null}
        onChange={(value) => setFolderNameDialog((current) => current ? { ...current, value } : current)}
        onCancel={() => setFolderNameDialog(null)}
        onSubmit={() => void handleFolderNameDialogSubmit()}
      />
    )}
    </>
  );
}

function FolderNameDialog({
  title,
  inputLabel,
  value,
  confirmLabel,
  disabled,
  pending,
  onChange,
  onCancel,
  onSubmit,
}: {
  title: string;
  inputLabel: string;
  value: string;
  confirmLabel: string;
  disabled: boolean;
  pending: boolean;
  onChange: (value: string) => void;
  onCancel: () => void;
  onSubmit: () => void;
}) {
  const { t } = useTranslation();
  const inputRef = useRef<HTMLInputElement>(null);
  const onCancelRef = useRef(onCancel);

  useEffect(() => { onCancelRef.current = onCancel; }, [onCancel]);

  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    inputRef.current?.focus();
    inputRef.current?.select();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        onCancelRef.current();
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      previousFocus?.focus();
    };
  }, []);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="folder-name-dialog-title"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onCancel();
        }
      }}
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "rgba(0,0,0,0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1100,
      }}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (!disabled) {
            onSubmit();
          }
        }}
        style={{
          width: "360px",
          backgroundColor: "var(--color-sidebar-bg)",
          color: "var(--color-text-primary)",
          border: "1px solid var(--color-border)",
          borderRadius: "8px",
          boxShadow: "0 20px 60px rgba(0,0,0,0.3)",
          padding: "20px",
          display: "flex",
          flexDirection: "column",
          gap: "14px",
        }}
      >
        <h3
          id="folder-name-dialog-title"
          style={{
            margin: 0,
            fontSize: "15px",
            fontWeight: 600,
            color: "var(--color-text-primary)",
          }}
        >
          {title}
        </h3>
        <label
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "6px",
            fontSize: "12px",
            color: "var(--color-text-secondary)",
          }}
        >
          <span>{inputLabel}</span>
          <input
            ref={inputRef}
            type="text"
            value={value}
            onChange={(event) => onChange(event.target.value)}
            disabled={pending}
            style={{
              width: "100%",
              boxSizing: "border-box",
              padding: "8px 10px",
              borderRadius: "6px",
              border: "1px solid var(--color-border)",
              backgroundColor: "var(--color-bg)",
              color: "var(--color-text-primary)",
              fontSize: "13px",
              outline: "none",
            }}
          />
        </label>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px" }}>
          <button
            type="button"
            onClick={onCancel}
            style={{
              padding: "7px 16px",
              borderRadius: "6px",
              border: "1px solid var(--color-border)",
              backgroundColor: "transparent",
              color: "var(--color-text-primary)",
              fontSize: "13px",
              cursor: "pointer",
            }}
          >
            {t("common.cancel", "Cancel")}
          </button>
          <button
            type="submit"
            disabled={disabled}
            style={{
              padding: "7px 16px",
              borderRadius: "6px",
              border: "none",
              backgroundColor: "var(--color-accent)",
              color: "#fff",
              fontSize: "13px",
              fontWeight: 600,
              cursor: disabled ? "default" : "pointer",
              opacity: disabled ? 0.45 : 1,
            }}
          >
            {confirmLabel}
          </button>
        </div>
      </form>
    </div>
  );
}

function FolderTreeItem({
  node,
  depth,
  path,
  activeFolderId,
  activeView,
  collapsed,
  collapsedPaths,
  onToggleCollapse,
  buttonBase,
  showUnread,
  folders,
  unreadCounts,
  onFolderClick,
  onRenameFolder,
  onDeleteFolder,
  onLinkFolder,
  onUnlinkFolder,
  actionFolderId,
  openFolderMenuId,
  onOpenFolderMenuChange,
  allowFolderActions,
}: {
  node: FolderTreeNode;
  depth: number;
  path: string;
  activeFolderId: string | null;
  activeView: string;
  collapsed: boolean;
  collapsedPaths: Set<string>;
  onToggleCollapse: (path: string) => void;
  buttonBase: React.CSSProperties;
  showUnread: boolean;
  folders: FolderType[];
  unreadCounts: Record<string, number>;
  onFolderClick: (id: string) => void;
  onRenameFolder: (folder: FolderType) => void;
  onDeleteFolder: (folder: FolderType) => void;
  onLinkFolder: (folder: FolderType) => void;
  onUnlinkFolder: (folder: FolderType) => void;
  actionFolderId: string | null;
  openFolderMenuId: string | null;
  onOpenFolderMenuChange: (folderId: string | null) => void;
  allowFolderActions: boolean;
}) {
  const { t } = useTranslation();
  const hasChildren = node.children.length > 0;
  const isCollapsed = collapsedPaths.has(path);
  const isActive = !!node.folder && node.folder.id === activeFolderId && activeView === "inbox";
  const indent = depth * 12;
  const canManageFolder = allowFolderActions && !!node.folder && !node.folder.role && !node.folder.is_system;
  const isActionPending = !!node.folder && actionFolderId === node.folder.id;
  const isMenuOpen = !!node.folder && openFolderMenuId === node.folder.id;

  const handleClick = () => {
    if (node.folder) {
      onFolderClick(node.folder.id);
    } else {
      onToggleCollapse(path);
    }
  };

  return (
    <>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          width: "100%",
        }}
      >
        <button
          type="button"
          aria-label={collapsed ? node.label : undefined}
          aria-current={isActive ? "page" : undefined}
          title={collapsed ? node.label : undefined}
          onClick={hasChildren && node.folder ? undefined : handleClick}
          style={{
            ...buttonBase,
            paddingLeft: collapsed ? undefined : `${10 + indent}px`,
            backgroundColor: isActive ? "var(--color-sidebar-active)" : "transparent",
            color: "var(--color-text-primary)",
            cursor: "pointer",
            transition: "background-color 0.15s ease",
            position: "relative",
            flex: 1,
            minWidth: 0,
          }}
          onMouseEnter={(e) => {
            if (!isActive) e.currentTarget.style.backgroundColor = "var(--color-sidebar-hover)";
          }}
          onMouseLeave={(e) => {
            if (!isActive) e.currentTarget.style.backgroundColor = "transparent";
          }}
        >
          {hasChildren && !collapsed && (
            <span
              onClick={(e) => { e.stopPropagation(); onToggleCollapse(path); }}
              style={{ display: "flex", alignItems: "center", flexShrink: 0, opacity: 0.5 }}
            >
              {isCollapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
            </span>
          )}
          {!(hasChildren && !collapsed) && (
            <Folder size={16} style={{ flexShrink: 0 }} />
          )}
          {!collapsed && (
            <>
              <span
                style={{ display: "flex", alignItems: "center", gap: "6px", minWidth: 0, flex: 1 }}
                onClick={node.folder ? () => onFolderClick(node.folder!.id) : () => onToggleCollapse(path)}
              >
                <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {node.label}
                </span>
                {node.folder && (
                  <span
                    title={node.folder.server_linked ? t("sidebar.serverSync", "Server sync") : t("sidebar.localOnly", "Local only")}
                    aria-label={node.folder.server_linked ? t("sidebar.serverSync", "Server sync") : t("sidebar.localOnly", "Local only")}
                    style={{
                      borderRadius: "999px",
                      border: "1px solid var(--color-border)",
                      color: node.folder.server_linked ? "var(--color-accent)" : "var(--color-text-secondary)",
                      fontSize: "9px",
                      fontWeight: 700,
                      lineHeight: "14px",
                      padding: "0 5px",
                      flexShrink: 0,
                    }}
                  >
                    {node.folder.server_linked ? "SYNC" : "LOCAL"}
                  </span>
                )}
              </span>
              {showUnread && node.folder && unreadCountForFolder(node.folder.id, folders, unreadCounts) > 0 && (
                <span style={{ fontSize: "11px", fontWeight: 600, color: "var(--color-accent)", minWidth: "18px", textAlign: "right" }}>
                  {unreadCountForFolder(node.folder.id, folders, unreadCounts)}
                </span>
              )}
            </>
          )}
        </button>
        {!collapsed && canManageFolder && (
          <span style={{ position: "relative", display: "flex", alignItems: "center", flexShrink: 0 }}>
            <button
              type="button"
              aria-label={t("sidebar.folderActions", "Folder actions")}
              aria-haspopup="menu"
              aria-expanded={isMenuOpen}
              title={t("sidebar.folderActions", "Folder actions")}
              disabled={isActionPending}
              onClick={(event) => {
                event.stopPropagation();
                onOpenFolderMenuChange(isMenuOpen ? null : node.folder!.id);
              }}
              style={{
                width: "22px",
                height: "22px",
                borderRadius: "6px",
                border: "none",
                backgroundColor: isMenuOpen ? "var(--color-sidebar-hover)" : "transparent",
                color: "var(--color-text-secondary)",
                cursor: isActionPending ? "default" : "pointer",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                opacity: isActionPending ? 0.45 : 1,
              }}
            >
              <MoreHorizontal size={15} />
            </button>
            {isMenuOpen && (
              <div
                role="menu"
                onClick={(event) => event.stopPropagation()}
                style={{
                  position: "absolute",
                  top: "calc(100% + 4px)",
                  right: 0,
                  minWidth: "132px",
                  padding: "4px",
                  borderRadius: "8px",
                  border: "1px solid var(--color-border)",
                  backgroundColor: "var(--color-bg)",
                  boxShadow: "0 4px 12px rgba(0,0,0,0.15)",
                  zIndex: 20,
                }}
              >
                <FolderActionMenuItem
                  label={t("sidebar.renameFolderAction", "Rename")}
                  disabled={isActionPending}
                  onClick={() => {
                    onOpenFolderMenuChange(null);
                    onRenameFolder(node.folder!);
                  }}
                >
                  <Pencil size={14} />
                </FolderActionMenuItem>
                <FolderActionMenuItem
                  label={node.folder!.server_linked ? t("sidebar.unlinkFolderAction", "Unlink") : t("sidebar.linkFolderAction", "Link")}
                  disabled={isActionPending}
                  onClick={() => {
                    onOpenFolderMenuChange(null);
                    if (node.folder!.server_linked) {
                      onUnlinkFolder(node.folder!);
                    } else {
                      onLinkFolder(node.folder!);
                    }
                  }}
                >
                  {node.folder!.server_linked ? <Link2Off size={14} /> : <Link size={14} />}
                </FolderActionMenuItem>
                <FolderActionMenuItem
                  label={t("sidebar.deleteFolderAction", "Delete")}
                  disabled={isActionPending}
                  onClick={() => {
                    onOpenFolderMenuChange(null);
                    onDeleteFolder(node.folder!);
                  }}
                >
                  <Trash2 size={14} />
                </FolderActionMenuItem>
              </div>
            )}
          </span>
        )}
      </div>
      {hasChildren && !isCollapsed && node.children.map((child) => (
        <FolderTreeItem
          key={child.folder?.id ?? `virt:${path}/${child.label}`}
          node={child}
          depth={depth + 1}
          path={`${path}/${child.label}`}
          activeFolderId={activeFolderId}
          activeView={activeView}
          collapsed={collapsed}
          collapsedPaths={collapsedPaths}
          onToggleCollapse={onToggleCollapse}
          buttonBase={buttonBase}
          showUnread={showUnread}
          folders={folders}
          unreadCounts={unreadCounts}
          onFolderClick={onFolderClick}
          onRenameFolder={onRenameFolder}
          onDeleteFolder={onDeleteFolder}
          onLinkFolder={onLinkFolder}
          onUnlinkFolder={onUnlinkFolder}
          actionFolderId={actionFolderId}
          openFolderMenuId={openFolderMenuId}
          onOpenFolderMenuChange={onOpenFolderMenuChange}
          allowFolderActions={allowFolderActions}
        />
      ))}
    </>
  );
}

function FolderActionMenuItem({
  children,
  disabled,
  label,
  onClick,
}: {
  children: React.ReactNode;
  disabled: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={(event) => {
        event.stopPropagation();
        onClick();
      }}
      style={{
        display: "flex",
        alignItems: "center",
        gap: "8px",
        width: "100%",
        padding: "7px 8px",
        borderRadius: "6px",
        border: "none",
        backgroundColor: "transparent",
        color: "var(--color-text-primary)",
        cursor: disabled ? "default" : "pointer",
        fontSize: "12px",
        textAlign: "left",
        opacity: disabled ? 0.45 : 1,
      }}
    >
      {children}
      <span>{label}</span>
    </button>
  );
}

// Reusable sidebar button to avoid repetitive hover logic
function SidebarButton({
  icon, label, badge, isActive, collapsed, style, disabled, onClick,
}: {
  icon: React.ReactNode;
  label: string;
  badge?: number;
  isActive: boolean;
  collapsed: boolean;
  style: React.CSSProperties;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={collapsed ? label : undefined}
      aria-current={isActive ? "page" : undefined}
      title={collapsed ? label : undefined}
      disabled={disabled}
      style={{
        ...style,
        backgroundColor: isActive
          ? "var(--color-sidebar-active)"
          : style.backgroundColor ?? "transparent",
        color: style.color ?? "var(--color-text-primary)",
        opacity: disabled ? 0.45 : 1,
        cursor: disabled ? "default" : "pointer",
        transition: "background-color 0.15s ease, opacity 0.15s ease",
      }}
      onMouseEnter={(e) => {
        if (!isActive && !style.backgroundColor)
          e.currentTarget.style.backgroundColor = "var(--color-sidebar-hover)";
      }}
      onMouseLeave={(e) => {
        if (!isActive && !style.backgroundColor)
          e.currentTarget.style.backgroundColor = "transparent";
      }}
    >
      {icon}
      {!collapsed && (
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", flex: 1 }}>
          {label}
        </span>
      )}
      {!collapsed && badge != null && badge > 0 && (
        <span style={{
          fontSize: "11px",
          fontWeight: 600,
          color: "var(--color-accent)",
          minWidth: "18px",
          textAlign: "right",
        }}>
          {badge}
        </span>
      )}
    </button>
  );
}
