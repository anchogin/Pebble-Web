use crate::Store;
use pebble_core::{now_timestamp, AppSettingsRecord, Result};
use rusqlite::params;

impl Store {
    pub fn save_app_settings(&self, settings: &AppSettingsRecord) -> Result<()> {
        let public_url = trim_trailing_slash(&settings.public_url);
        self.with_write(|conn| {
            conn.execute(
                "INSERT INTO app_settings (id, public_url, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                     public_url = excluded.public_url,
                     updated_at = excluded.updated_at",
                params![
                    settings.id,
                    public_url,
                    settings.created_at,
                    settings.updated_at
                ],
            )?;
            Ok(())
        })
    }

    pub fn get_app_settings(&self) -> Result<AppSettingsRecord> {
        self.with_read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, public_url, created_at, updated_at FROM app_settings WHERE id = 'active'",
            )?;
            let mut rows = stmt.query_map([], |row| {
                Ok(AppSettingsRecord {
                    id: row.get(0)?,
                    public_url: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?;
            match rows.next() {
                Some(settings) => Ok(settings?),
                None => {
                    let now = now_timestamp();
                    Ok(AppSettingsRecord {
                        id: "active".to_string(),
                        public_url: String::new(),
                        created_at: now,
                        updated_at: now,
                    })
                }
            }
        })
    }
}

fn trim_trailing_slash(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use crate::Store;
    use pebble_core::{now_timestamp, AppSettingsRecord};

    #[test]
    fn app_settings_default_to_empty_public_url() {
        let store = Store::open_in_memory().unwrap();

        let settings = store.get_app_settings().unwrap();

        assert_eq!(settings.public_url, "");
    }

    #[test]
    fn app_settings_save_and_load_trimmed_public_url() {
        let store = Store::open_in_memory().unwrap();
        let now = now_timestamp();

        store
            .save_app_settings(&AppSettingsRecord {
                id: "active".to_string(),
                public_url: "https://mail.example.com/".to_string(),
                created_at: now,
                updated_at: now,
            })
            .unwrap();

        let settings = store.get_app_settings().unwrap();

        assert_eq!(settings.public_url, "https://mail.example.com");
    }
}
