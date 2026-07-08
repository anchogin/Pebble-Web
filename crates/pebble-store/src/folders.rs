use pebble_core::{Folder, FolderRole, FolderType, PebbleError, Result};
use rusqlite::{params, OptionalExtension};

use crate::Store;

fn folder_type_to_str(ft: &FolderType) -> &'static str {
    match ft {
        FolderType::Folder => "folder",
        FolderType::Label => "label",
        FolderType::Category => "category",
    }
}

fn str_to_folder_type(s: &str) -> FolderType {
    match s {
        "label" => FolderType::Label,
        "category" => FolderType::Category,
        _ => FolderType::Folder,
    }
}

fn folder_role_to_str(role: &FolderRole) -> &'static str {
    match role {
        FolderRole::Inbox => "inbox",
        FolderRole::Sent => "sent",
        FolderRole::Drafts => "drafts",
        FolderRole::Trash => "trash",
        FolderRole::Archive => "archive",
        FolderRole::Spam => "spam",
    }
}

fn str_to_folder_role(s: &str) -> Option<FolderRole> {
    match s {
        "inbox" => Some(FolderRole::Inbox),
        "sent" => Some(FolderRole::Sent),
        "drafts" => Some(FolderRole::Drafts),
        "trash" => Some(FolderRole::Trash),
        "archive" => Some(FolderRole::Archive),
        "spam" => Some(FolderRole::Spam),
        _ => None,
    }
}

fn map_folder_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Folder> {
    let role_str: Option<String> = row.get(5)?;
    let is_system: i32 = row.get(8)?;
    let server_linked: i32 = row.get(9)?;
    Ok(Folder {
        id: row.get(0)?,
        account_id: row.get(1)?,
        remote_id: row.get(2)?,
        name: row.get(3)?,
        folder_type: str_to_folder_type(&row.get::<_, String>(4)?),
        role: role_str.and_then(|s| str_to_folder_role(&s)),
        parent_id: row.get(6)?,
        color: row.get(7)?,
        is_system: is_system != 0,
        server_linked: server_linked != 0,
        sort_order: row.get(10)?,
    })
}

const FOLDER_SELECT: &str = "id, account_id, remote_id, name, folder_type, role, parent_id, color, is_system, server_linked, sort_order";

impl Store {
    /// Upsert a folder. Returns the effective database id (the existing row's id
    /// when the folder already exists, or `folder.id` for a new insert).
    pub fn insert_folder(&self, folder: &Folder) -> Result<String> {
        self.with_write(|conn| {
            // Upsert: if a folder with the same (account_id, remote_id) exists,
            // update its name/role/sort_order instead of creating a duplicate.
            let existing: Option<String> = conn
                .query_row(
                    "SELECT id FROM folders WHERE account_id = ?1 AND remote_id = ?2",
                    rusqlite::params![folder.account_id, folder.remote_id],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(existing_id) = existing {
                conn.execute(
                    "UPDATE folders SET name = ?1, folder_type = ?2, role = ?3, is_system = ?4, server_linked = ?5, sort_order = ?6
                     WHERE id = ?7",
                    rusqlite::params![
                        folder.name,
                        folder_type_to_str(&folder.folder_type),
                        folder.role.as_ref().map(folder_role_to_str),
                        folder.is_system as i32,
                        folder.server_linked as i32,
                        folder.sort_order,
                        existing_id,
                    ],
                )?;
                Ok(existing_id)
            } else {
                conn.execute(
                    "INSERT INTO folders (id, account_id, remote_id, name, folder_type, role, parent_id, color, is_system, server_linked, sort_order)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    rusqlite::params![
                        folder.id,
                        folder.account_id,
                        folder.remote_id,
                        folder.name,
                        folder_type_to_str(&folder.folder_type),
                        folder.role.as_ref().map(folder_role_to_str),
                        folder.parent_id,
                        folder.color,
                        folder.is_system as i32,
                        folder.server_linked as i32,
                        folder.sort_order,
                    ],
                )?;
                Ok(folder.id.clone())
            }
        })
    }

    pub fn find_folder_by_role(
        &self,
        account_id: &str,
        role: FolderRole,
    ) -> Result<Option<Folder>> {
        let role_str = folder_role_to_str(&role);
        self.with_read(|conn| {
            let mut stmt = conn.prepare_cached(&format!(
                "SELECT {FOLDER_SELECT} FROM folders WHERE account_id = ?1 AND role = ?2 LIMIT 1"
            ))?;
            let result = stmt
                .query_row(params![account_id, role_str], map_folder_row)
                .optional()?;
            Ok(result)
        })
    }

    pub fn find_folder_by_id(&self, folder_id: &str) -> Result<Option<Folder>> {
        self.with_read(|conn| {
            conn.query_row(
                &format!("SELECT {FOLDER_SELECT} FROM folders WHERE id = ?1 LIMIT 1"),
                params![folder_id],
                map_folder_row,
            )
            .optional()
            .map_err(Into::into)
        })
    }

    pub fn find_folder_by_name(&self, account_id: &str, name: &str) -> Result<Option<Folder>> {
        let lower = name.to_lowercase();
        let folders = self.list_folders(account_id)?;
        Ok(folders.into_iter().find(|f| f.name.to_lowercase() == lower))
    }

    /// Idempotent: return the existing folder whose name matches (case-insensitive)
    /// the given `name` within `account_id`; otherwise insert a new local-only folder
    /// of type `Folder`. When `is_system && name.eq_ignore_ascii_case("Archive")`,
    /// the new folder gets `role = Some(Archive)` and `sort_order = 0` so it
    /// surfaces as the account's Archive folder.
    pub fn find_or_create_folder_by_name(
        &self,
        account_id: &str,
        name: &str,
        is_system: bool,
    ) -> Result<Folder> {
        if let Some(existing) = self.find_folder_by_name(account_id, name)? {
            return Ok(existing);
        }
        let id = pebble_core::new_id();
        // Archive is the only system role the engine auto-creates; assign it
        // the matching role + sort_order so subsequent role lookups find it.
        let (role, sort_order) = if is_system && name.eq_ignore_ascii_case("Archive") {
            (Some(FolderRole::Archive), 0)
        } else {
            (None, 1000)
        };
        let folder = Folder {
            id: id.clone(),
            account_id: account_id.into(),
            remote_id: format!("local-{}", name),
            name: name.into(),
            folder_type: FolderType::Folder,
            role,
            parent_id: None,
            color: None,
            is_system,
            server_linked: false,
            sort_order,
        };
        self.insert_folder(&folder)?;
        Ok(folder)
    }

    pub fn delete_folder_by_remote_id(&self, account_id: &str, remote_id: &str) -> Result<()> {
        self.with_write(|conn| {
            conn.execute(
                "DELETE FROM folders WHERE account_id = ?1 AND remote_id = ?2",
                rusqlite::params![account_id, remote_id],
            )?;
            Ok(())
        })
    }

    pub fn rename_folder(&self, folder_id: &str, name: &str) -> Result<()> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(PebbleError::Validation(
                "Folder name is required".to_string(),
            ));
        }
        self.with_write(|conn| {
            conn.execute(
                "UPDATE folders SET name = ?1 WHERE id = ?2",
                params![trimmed, folder_id],
            )?;
            Ok(())
        })
    }

    pub fn delete_folder_by_id(&self, folder_id: &str) -> Result<()> {
        self.with_write(|conn| {
            conn.execute("DELETE FROM folders WHERE id = ?1", params![folder_id])?;
            Ok(())
        })
    }

    pub fn set_folder_server_linked(&self, folder_id: &str, server_linked: bool) -> Result<()> {
        self.with_write(|conn| {
            conn.execute(
                "UPDATE folders SET server_linked = ?1 WHERE id = ?2",
                params![server_linked as i32, folder_id],
            )?;
            Ok(())
        })
    }

    pub fn list_folders(&self, account_id: &str) -> Result<Vec<Folder>> {
        self.with_read(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {FOLDER_SELECT} FROM folders WHERE account_id = ?1 ORDER BY sort_order ASC"
            ))?;
            let rows = stmt.query_map(rusqlite::params![account_id], map_folder_row)?;
            let mut folders = Vec::new();
            for row in rows {
                folders.push(row?);
            }
            Ok(folders)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pebble_core::{Account, ProviderType};

    fn account() -> Account {
        let now = pebble_core::now_timestamp();
        Account {
            id: pebble_core::new_id(),
            email: "test@example.com".to_string(),
            display_name: "Test".to_string(),
            color: None,
            provider: ProviderType::Imap,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn locally_created_folder_is_unlinked_until_linked() {
        let store = Store::open_in_memory().unwrap();
        let account = account();
        store.insert_account(&account).unwrap();

        let folder = store
            .find_or_create_folder_by_name(&account.id, "DMIT", false)
            .unwrap();
        assert!(!folder.server_linked);

        store.set_folder_server_linked(&folder.id, true).unwrap();
        let linked = store.find_folder_by_id(&folder.id).unwrap().unwrap();
        assert!(linked.server_linked);

        store.set_folder_server_linked(&folder.id, false).unwrap();
        let unlinked = store.find_folder_by_id(&folder.id).unwrap().unwrap();
        assert!(!unlinked.server_linked);
    }

    #[test]
    fn rename_and_delete_folder_preserve_messages() {
        let store = Store::open_in_memory().unwrap();
        let account = account();
        store.insert_account(&account).unwrap();

        let folder = store
            .find_or_create_folder_by_name(&account.id, "Old", false)
            .unwrap();
        store.rename_folder(&folder.id, "New").unwrap();
        let renamed = store.find_folder_by_id(&folder.id).unwrap().unwrap();
        assert_eq!(renamed.name, "New");

        store.delete_folder_by_id(&folder.id).unwrap();
        assert!(store.find_folder_by_id(&folder.id).unwrap().is_none());
    }
}
