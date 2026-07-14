use crate::Store;
use pebble_core::{MagicPushConfigRecord, PebbleError, Result};
use rusqlite::params;

impl Store {
    pub(crate) fn save_magicpush_config_with_conn(
        conn: &rusqlite::Connection,
        config: &MagicPushConfigRecord,
    ) -> Result<()> {
        conn.execute(
            "INSERT INTO magicpush_config (id, base_url, token_encrypted, public_url, is_enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                 base_url = excluded.base_url,
                 token_encrypted = excluded.token_encrypted,
                 public_url = excluded.public_url,
                 is_enabled = excluded.is_enabled,
                 updated_at = excluded.updated_at",
            params![
                config.id,
                config.base_url,
                config.token_encrypted,
                config.public_url,
                config.is_enabled as i32,
                config.created_at,
                config.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn save_magicpush_config(&self, config: &MagicPushConfigRecord) -> Result<()> {
        self.with_write(|conn| Self::save_magicpush_config_with_conn(conn, config))
    }

    pub fn get_magicpush_config(&self) -> Result<Option<MagicPushConfigRecord>> {
        self.with_read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, base_url, token_encrypted, public_url, is_enabled, created_at, updated_at
                 FROM magicpush_config WHERE id = 'active'",
            )?;
            let mut rows = stmt.query_map([], |row| {
                Ok(MagicPushConfigRecord {
                    id: row.get(0)?,
                    base_url: row.get(1)?,
                    token_encrypted: row.get(2)?,
                    public_url: row.get(3)?,
                    is_enabled: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            match rows.next() {
                Some(Ok(config)) => Ok(Some(config)),
                Some(Err(e)) => Err(PebbleError::Storage(e.to_string())),
                None => Ok(None),
            }
        })
    }

    pub fn delete_magicpush_config(&self) -> Result<()> {
        self.with_write(|conn| {
            conn.execute("DELETE FROM magicpush_config WHERE id = 'active'", [])?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pebble_core::now_timestamp;

    #[test]
    fn magicpush_config_save_and_load() {
        let store = Store::open_in_memory().unwrap();
        let now = now_timestamp();
        let config = MagicPushConfigRecord {
            id: "active".to_string(),
            base_url: "https://push.example.com".to_string(),
            token_encrypted: Some("abcd".to_string()),
            public_url: "https://mail.example.com".to_string(),
            is_enabled: true,
            created_at: now,
            updated_at: now,
        };

        store.save_magicpush_config(&config).unwrap();

        let loaded = store.get_magicpush_config().unwrap().unwrap();
        assert_eq!(loaded.base_url, "https://push.example.com");
        assert_eq!(loaded.token_encrypted.as_deref(), Some("abcd"));
        assert_eq!(loaded.public_url, "https://mail.example.com");
        assert!(loaded.is_enabled);
    }

    #[test]
    fn magicpush_config_upsert_and_delete() {
        let store = Store::open_in_memory().unwrap();
        let now = now_timestamp();
        store
            .save_magicpush_config(&MagicPushConfigRecord {
                id: "active".to_string(),
                base_url: "https://old.example.com".to_string(),
                token_encrypted: Some("old".to_string()),
                public_url: "https://mail-old.example.com".to_string(),
                is_enabled: true,
                created_at: now,
                updated_at: now,
            })
            .unwrap();
        store
            .save_magicpush_config(&MagicPushConfigRecord {
                id: "active".to_string(),
                base_url: "https://new.example.com".to_string(),
                token_encrypted: None,
                public_url: "https://mail-new.example.com".to_string(),
                is_enabled: false,
                created_at: now,
                updated_at: now + 1,
            })
            .unwrap();

        let loaded = store.get_magicpush_config().unwrap().unwrap();
        assert_eq!(loaded.base_url, "https://new.example.com");
        assert_eq!(loaded.token_encrypted, None);
        assert!(!loaded.is_enabled);

        store.delete_magicpush_config().unwrap();
        assert!(store.get_magicpush_config().unwrap().is_none());
    }
}
