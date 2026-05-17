import api from "../api-client";

// Re-export all IPC types so existing `import { Foo } from "@/lib/api"` keeps working.
export type {
  Account,
  AccountProxyMode,
  AccountProxySetting,
  AddAccountRequest,
  AdvancedSearchQuery,
  AppLogSnapshot,
  Attachment,
  BackupPreview,
  ConnectionSecurity,
  EmailAddress,
  Folder,
  HttpProxyConfig,
  ImportedBackgroundImage,
  KanbanCard,
  KanbanColumnType,
  KnownContact,
  Label,
  Message,
  MessageSummary,
  NotificationStatus,
  PendingMailOp,
  PendingMailOpsSummary,
  PrivacyMode,
  RenderedHtml,
  Rule,
  SearchHit,
  SnoozedMessage,
  ThreadSummary,
  TranslateConfig,
  TranslateResult,
  TrustedSender,
} from "./ipc-types";

import type {
  Account,
  AccountProxyMode,
  AccountProxySetting,
  AddAccountRequest,
  AdvancedSearchQuery,
  AppLogSnapshot,
  Attachment,
  BackupPreview,
  ConnectionSecurity,
  Folder,
  HttpProxyConfig,
  KanbanCard,
  KanbanColumnType,
  KnownContact,
  Label,
  Message,
  MessageSummary,
  NotificationStatus,
  PendingMailOp,
  PendingMailOpsSummary,
  PrivacyMode,
  RenderedHtml,
  Rule,
  SearchHit,
  SnoozedMessage,
  ThreadSummary,
  TranslateConfig,
  TranslateResult,
  TrustedSender,
} from "./ipc-types";

// ─── Account API ─────────────────────────────────────────────────────────────

export async function healthCheck(): Promise<string> {
  const res = await api.get<string>("/health");
  return res.data;
}

export async function readAppLog(_maxBytes: number): Promise<AppLogSnapshot> {
  // Not implemented in web backend
  return { path: "", content: "", truncated: false };
}

export async function getGlobalProxy(): Promise<HttpProxyConfig | null> {
  throw new Error("Not implemented: getGlobalProxy");
}

export async function getAccountProxy(_accountId: string): Promise<HttpProxyConfig | null> {
  throw new Error("Not implemented: getAccountProxy");
}

export async function getAccountProxySetting(_accountId: string): Promise<AccountProxySetting> {
  throw new Error("Not implemented: getAccountProxySetting");
}

export async function updateAccountProxy(
  _accountId: string,
  _proxyHost?: string,
  _proxyPort?: number,
): Promise<void> {
  throw new Error("Not implemented: updateAccountProxy");
}

export async function updateAccountProxySetting(
  _accountId: string,
  _mode: AccountProxyMode,
  _proxyHost?: string,
  _proxyPort?: number,
): Promise<void> {
  throw new Error("Not implemented: updateAccountProxySetting");
}

export async function updateGlobalProxy(
  _proxyHost?: string,
  _proxyPort?: number,
): Promise<void> {
  throw new Error("Not implemented: updateGlobalProxy");
}

export async function completeOAuthFlow(
  _provider: string,
  _email: string,
  _displayName: string,
  _proxyHost?: string,
  _proxyPort?: number,
): Promise<Account> {
  throw new Error("Not implemented: completeOAuthFlow");
}

export async function getOAuthAccountProxy(_accountId: string): Promise<HttpProxyConfig | null> {
  throw new Error("Not implemented: getOAuthAccountProxy");
}

export async function getOAuthAccountProxySetting(_accountId: string): Promise<AccountProxySetting> {
  throw new Error("Not implemented: getOAuthAccountProxySetting");
}

export async function updateOAuthAccountProxy(
  _accountId: string,
  _proxyHost?: string,
  _proxyPort?: number,
): Promise<void> {
  throw new Error("Not implemented: updateOAuthAccountProxy");
}

export async function updateOAuthAccountProxySetting(
  _accountId: string,
  _mode: AccountProxyMode,
  _proxyHost?: string,
  _proxyPort?: number,
): Promise<void> {
  throw new Error("Not implemented: updateOAuthAccountProxySetting");
}

export async function addAccount(_request: AddAccountRequest): Promise<Account> {
  throw new Error("Not implemented: addAccount");
}

export async function testAccountConnection(_accountId: string): Promise<string> {
  throw new Error("Not implemented: testAccountConnection");
}

export async function testImapConnection(
  _imapHost: string,
  _imapPort: number,
  _imapSecurity: ConnectionSecurity,
  _proxyHost?: string,
  _proxyPort?: number,
  _username?: string,
  _password?: string,
): Promise<string> {
  throw new Error("Not implemented: testImapConnection");
}

export async function listAccounts(): Promise<Account[]> {
  const res = await api.get<Account[]>("/accounts");
  return res.data;
}

export async function updateAccount(
  _accountId: string,
  _email: string,
  _displayName: string,
  _password?: string,
  _imapHost?: string,
  _imapPort?: number,
  _smtpHost?: string,
  _smtpPort?: number,
  _imapSecurity?: ConnectionSecurity,
  _smtpSecurity?: ConnectionSecurity,
  _proxyHost?: string,
  _proxyPort?: number,
  _accountColor?: string,
): Promise<void> {
  throw new Error("Not implemented: updateAccount");
}

export async function deleteAccount(_accountId: string): Promise<void> {
  throw new Error("Not implemented: deleteAccount");
}

// ─── Folder API ──────────────────────────────────────────────────────────────

export async function listFolders(accountId: string): Promise<Folder[]> {
  const res = await api.get<Folder[]>(`/accounts/${accountId}/folders`);
  return res.data;
}

// ─── Message API ─────────────────────────────────────────────────────────────

export async function listMessages(
  folderId: string,
  limit: number,
  offset: number,
  _folderIds?: string[],
): Promise<MessageSummary[]> {
  const res = await api.get<MessageSummary[]>(`/folders/${folderId}/messages`, {
    params: { limit, offset },
  });
  return res.data;
}

export async function listStarredMessages(
  accountId: string,
  limit: number,
  offset: number,
): Promise<MessageSummary[]> {
  const res = await api.get<MessageSummary[]>(`/accounts/${accountId}/starred`, {
    params: { limit, offset },
  });
  return res.data;
}

export async function getMessage(messageId: string): Promise<Message | null> {
  const res = await api.get<Message | null>(`/messages/${messageId}`);
  return res.data;
}

export async function getMessagesBatch(messageIds: string[]): Promise<Message[]> {
  const res = await api.post<Message[]>("/messages/batch", { messageIds });
  return res.data;
}

export async function getRenderedHtml(
  messageId: string,
  privacyMode: PrivacyMode,
): Promise<RenderedHtml> {
  const res = await api.post<RenderedHtml>(`/messages/${messageId}/render`, { privacyMode });
  return res.data;
}

export async function getMessageWithHtml(
  messageId: string,
  privacyMode: PrivacyMode,
): Promise<[Message, RenderedHtml] | null> {
  const res = await api.post<[Message, RenderedHtml] | null>(`/messages/${messageId}/with-html`, { privacyMode });
  return res.data;
}

export async function updateMessageFlags(
  messageId: string,
  isRead?: boolean,
  isStarred?: boolean,
): Promise<void> {
  await api.put(`/messages/${messageId}/flags`, { isRead, isStarred });
}

const archivingIds = new Set<string>();

export async function archiveMessage(messageId: string): Promise<string> {
  if (archivingIds.has(messageId)) {
    return "skipped";
  }
  archivingIds.add(messageId);
  try {
    const res = await api.post<string>(`/messages/${messageId}/archive`);
    return res.data;
  } finally {
    archivingIds.delete(messageId);
  }
}

export async function deleteMessage(messageId: string): Promise<void> {
  await api.delete(`/messages/${messageId}`);
}

export async function restoreMessage(messageId: string): Promise<void> {
  await api.post(`/messages/${messageId}/restore`);
}

export async function moveToFolder(messageId: string, targetFolderId: string): Promise<void> {
  await api.post(`/messages/${messageId}/move`, { folderId: targetFolderId });
}

export async function emptyTrash(accountId: string): Promise<number> {
  const res = await api.post<number>(`/accounts/${accountId}/empty-trash`);
  return res.data;
}

export async function getPendingMailOpsSummary(
  accountId: string | null,
): Promise<PendingMailOpsSummary> {
  const res = await api.get<PendingMailOpsSummary>("/pending-ops/summary", {
    params: { accountId },
  });
  return res.data;
}

export async function listPendingMailOps(
  accountId: string | null,
  limit = 100,
): Promise<PendingMailOp[]> {
  const res = await api.get<PendingMailOp[]>("/pending-ops", {
    params: { accountId, limit },
  });
  return res.data;
}

// ─── Trusted Senders API ────────────────────────────────────────────────────

export async function listTrustedSenders(_accountId: string): Promise<TrustedSender[]> {
  throw new Error("Not implemented: listTrustedSenders");
}

export async function removeTrustedSender(_accountId: string, _email: string): Promise<void> {
  throw new Error("Not implemented: removeTrustedSender");
}

export async function trustSender(_accountId: string, _email: string, _trustType: "images" | "all"): Promise<void> {
  throw new Error("Not implemented: trustSender");
}

export async function isTrustedSender(_accountId: string, _email: string): Promise<boolean> {
  throw new Error("Not implemented: isTrustedSender");
}

// ─── Search API ──────────────────────────────────────────────────────────────

export async function searchMessages(
  query: string,
  limit?: number,
): Promise<SearchHit[]> {
  const res = await api.post<SearchHit[]>("/search", { query, limit });
  return res.data;
}

export async function advancedSearch(
  query: AdvancedSearchQuery,
  limit?: number,
): Promise<SearchHit[]> {
  const res = await api.post<SearchHit[]>("/search/advanced", { query, limit });
  return res.data;
}

// ─── Sync API ────────────────────────────────────────────────────────────────

export async function startSync(accountId: string, _pollIntervalSecs?: number): Promise<string> {
  const res = await api.post<string>("/sync/trigger", { accountId, reason: "start_sync" });
  return res.data;
}

export async function triggerSync(accountId: string, reason: string): Promise<void> {
  await api.post("/sync/trigger", { accountId, reason });
}

export type RealtimePreference = "realtime" | "balanced" | "battery" | "manual";

export async function setRealtimePreference(_mode: RealtimePreference): Promise<void> {
  // No-op in web — realtime preference is a desktop feature
}

export async function setNotificationsEnabled(_enabled: boolean): Promise<void> {
  // No-op in web — desktop notifications not supported
}

export async function getNotificationStatus(): Promise<NotificationStatus> {
  return { enabled: false, attention_active: false, platform: "web", app_id: null };
}

export async function showTestNotification(): Promise<void> {
  // No-op in web
}

export async function clearNotificationAttention(): Promise<void> {
  // No-op in web
}

export async function setTrayMenuLabels(_showLabel: string, _hideLabel: string, _quitLabel: string): Promise<void> {
  // No-op in web — no system tray
}

export async function stopSync(_accountId: string): Promise<void> {
  // No-op in web
}

// ─── Attachment API ──────────────────────────────────────────────────────────

export async function listAttachments(messageId: string): Promise<Attachment[]> {
  const res = await api.get<Attachment[]>(`/messages/${messageId}/attachments`);
  return res.data;
}

export async function getAttachmentPath(_attachmentId: string): Promise<string | null> {
  throw new Error("Not implemented: getAttachmentPath (desktop-only)");
}

export async function downloadAttachment(_attachmentId: string, _saveTo: string): Promise<string> {
  throw new Error("Not implemented: downloadAttachment (desktop-only)");
}

// ─── Kanban API ──────────────────────────────────────────────────────────────

export async function moveToKanban(_messageId: string, _column: KanbanColumnType, _position?: number): Promise<void> {
  throw new Error("Not implemented: moveToKanban");
}

export async function listKanbanCards(_column?: KanbanColumnType): Promise<KanbanCard[]> {
  return [];
}

export async function removeFromKanban(_messageId: string): Promise<void> {
  throw new Error("Not implemented: removeFromKanban");
}

export async function listKanbanContextNotes(): Promise<Record<string, string>> {
  return {};
}

export async function setKanbanContextNote(
  _messageId: string,
  _note: string,
): Promise<Record<string, string>> {
  throw new Error("Not implemented: setKanbanContextNote");
}

export async function mergeKanbanContextNotes(
  _notes: Record<string, string>,
): Promise<Record<string, string>> {
  throw new Error("Not implemented: mergeKanbanContextNotes");
}

// ─── Snooze API ──────────────────────────────────────────────────────────────

export async function snoozeMessage(_messageId: string, _until: number, _returnTo: string): Promise<void> {
  throw new Error("Not implemented: snoozeMessage");
}

export async function unsnoozeMessage(_messageId: string): Promise<void> {
  throw new Error("Not implemented: unsnoozeMessage");
}

export async function listSnoozed(): Promise<SnoozedMessage[]> {
  return [];
}

// ─── Rules API ───────────────────────────────────────────────────────────────

export async function createRule(_name: string, _priority: number, _conditions: string, _actions: string): Promise<Rule> {
  throw new Error("Not implemented: createRule");
}

export async function listRules(): Promise<Rule[]> {
  return [];
}

export async function updateRule(_rule: Rule): Promise<void> {
  throw new Error("Not implemented: updateRule");
}

export async function deleteRule(_ruleId: string): Promise<void> {
  throw new Error("Not implemented: deleteRule");
}

// ─── Compose API ─────────────────────────────────────────────────────────────

export async function sendEmail(
  accountId: string,
  to: string[],
  cc: string[],
  bcc: string[],
  subject: string,
  bodyText: string,
  bodyHtml?: string,
  inReplyTo?: string,
  attachmentPaths?: string[],
): Promise<void> {
  await api.post("/compose", {
    accountId, to, cc, bcc, subject, bodyText, bodyHtml, inReplyTo, attachmentPaths,
  });
}

export async function stageComposeAttachment(_filename: string, _bytes: number[]): Promise<string> {
  throw new Error("Not implemented: stageComposeAttachment");
}

// ─── Batch Operations ───────────────────────────────────────────────────────

export async function batchArchive(messageIds: string[]): Promise<number> {
  const res = await api.post<number>("/messages/batch/archive", { messageIds });
  return res.data;
}

export async function batchDelete(messageIds: string[]): Promise<number> {
  const res = await api.post<number>("/messages/batch/delete", { messageIds });
  return res.data;
}

export async function batchMarkRead(messageIds: string[], isRead: boolean): Promise<number> {
  const res = await api.post<number>("/messages/batch/mark-read", { messageIds, isRead });
  return res.data;
}

export async function batchStar(messageIds: string[], starred: boolean): Promise<number> {
  const res = await api.post<number>("/messages/batch/star", { messageIds, starred });
  return res.data;
}

// ─── Translate API ───────────────────────────────────────────────────────────

export async function translateText(_text: string, _fromLang: string, _toLang: string): Promise<TranslateResult> {
  throw new Error("Not implemented: translateText");
}

export async function getTranslateConfig(): Promise<TranslateConfig | null> {
  return null;
}

export async function saveTranslateConfig(_providerType: string, _config: string, _isEnabled: boolean): Promise<void> {
  throw new Error("Not implemented: saveTranslateConfig");
}

export async function testTranslateConnection(_config: string): Promise<string> {
  throw new Error("Not implemented: testTranslateConnection");
}

// ─── Thread API ──────────────────────────────────────────────────────────────

export async function listThreads(
  folderId: string,
  limit: number,
  offset: number,
  _folderIds?: string[],
): Promise<ThreadSummary[]> {
  const res = await api.get<ThreadSummary[]>(`/folders/${folderId}/threads`, {
    params: { limit, offset },
  });
  return res.data;
}

export async function listThreadMessages(threadId: string): Promise<Message[]> {
  const res = await api.get<Message[]>(`/threads/${threadId}/messages`);
  return res.data;
}

// ─── Labels API ──────────────────────────────────────────────────────────────

export async function getMessageLabels(_messageId: string): Promise<Label[]> {
  return [];
}

export async function getMessageLabelsBatch(_messageIds: string[]): Promise<Record<string, Label[]>> {
  return {};
}

export async function addMessageLabel(_messageId: string, _labelName: string): Promise<void> {
  throw new Error("Not implemented: addMessageLabel");
}

export async function removeMessageLabel(_messageId: string, _labelName: string): Promise<void> {
  throw new Error("Not implemented: removeMessageLabel");
}

export async function listLabels(): Promise<Label[]> {
  return [];
}

// ─── Cloud Sync API ─────────────────────────────────────────────────────────

export async function testWebdavConnection(_url: string, _username: string, _password: string): Promise<string> {
  throw new Error("Not implemented: testWebdavConnection");
}

export async function backupToWebdav(_url: string, _username: string, _password: string): Promise<string> {
  throw new Error("Not implemented: backupToWebdav");
}

export async function previewWebdavBackup(_url: string, _username: string, _password: string): Promise<BackupPreview> {
  throw new Error("Not implemented: previewWebdavBackup");
}

export async function restoreFromWebdav(_url: string, _username: string, _password: string): Promise<string> {
  throw new Error("Not implemented: restoreFromWebdav");
}

// ─── Contacts API ────────────────────────────────────────────────────────────

export async function searchContacts(
  _accountId: string,
  _query: string,
  _limit?: number,
): Promise<KnownContact[]> {
  return [];
}

// ─── Drafts API ──────────────────────────────────────────────────────────────

export async function saveDraft(_args: {
  accountId: string;
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  bodyText: string;
  bodyHtml?: string;
  inReplyTo?: string;
  existingDraftId?: string;
  attachmentPaths?: string[];
}): Promise<string> {
  throw new Error("Not implemented: saveDraft");
}

export async function deleteDraft(_accountId: string, _draftId: string): Promise<void> {
  throw new Error("Not implemented: deleteDraft");
}

// ─── Folder Counts API ───────────────────────────────────────────────────────

export async function getFolderUnreadCounts(accountId: string): Promise<Record<string, number>> {
  const res = await api.get<Record<string, number>>(`/accounts/${accountId}/folder-unread-counts`);
  return res.data;
}
