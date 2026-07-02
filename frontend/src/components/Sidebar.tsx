import { useEffect, useMemo, useState } from "react";
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
} from "lucide-react";
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

const EMPTY_ACCOUNTS: Account[] = [];
const EMPTY_FOLDERS: FolderType[] = [];

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

  const toggleCollapsed = (path: string) => {
    setCollapsedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

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

  return (
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
          padding: "12px 10px 4px 10px",
          fontSize: "11px",
          fontWeight: 600,
          color: "var(--color-text-secondary)",
          textTransform: "uppercase",
          letterSpacing: "0.5px",
        }}>
          {t("sidebar.mail", "Mail")}
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
}) {
  const hasChildren = node.children.length > 0;
  const isCollapsed = collapsedPaths.has(path);
  const isActive = !!node.folder && node.folder.id === activeFolderId && activeView === "inbox";
  const indent = depth * 12;

  const handleClick = () => {
    if (node.folder) {
      onFolderClick(node.folder.id);
    } else {
      onToggleCollapse(path);
    }
  };

  return (
    <>
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
              style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", flex: 1 }}
              onClick={node.folder ? () => onFolderClick(node.folder!.id) : () => onToggleCollapse(path)}
            >
              {node.label}
            </span>
            {showUnread && node.folder && unreadCountForFolder(node.folder.id, folders, unreadCounts) > 0 && (
              <span style={{ fontSize: "11px", fontWeight: 600, color: "var(--color-accent)", minWidth: "18px", textAlign: "right" }}>
                {unreadCountForFolder(node.folder.id, folders, unreadCounts)}
              </span>
            )}
          </>
        )}
      </button>
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
        />
      ))}
    </>
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
