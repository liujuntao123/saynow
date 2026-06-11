use std::sync::Mutex;

use chrono::Utc;
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

use crate::models::{
    PersonalizationPreferences, RecognitionRecord, RecognitionStatus, StylePrompt, VocabularyItem,
};

const DEFAULT_HOTKEY: &str = "Alt";

pub struct AppDb {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key_ref: String,
    pub hotkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub id: i64,
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key_ref: String,
    pub enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: String::new(),
            base_url: String::new(),
            model: String::new(),
            api_key_ref: String::new(),
            hotkey: DEFAULT_HOTKEY.to_string(),
        }
    }
}

impl AppDb {
    pub fn in_memory() -> Result<Self> {
        let db = Self {
            conn: Mutex::new(Connection::open_in_memory()?),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn open(path: &std::path::Path) -> Result<Self> {
        let db = Self {
            conn: Mutex::new(Connection::open(path)?),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS app_config (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                provider TEXT NOT NULL,
                base_url TEXT NOT NULL,
                model TEXT NOT NULL,
                api_key_ref TEXT NOT NULL,
                hotkey TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS provider_configs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider TEXT NOT NULL,
                base_url TEXT NOT NULL,
                model TEXT NOT NULL,
                api_key_ref TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS recognition_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TEXT NOT NULL,
                duration_seconds INTEGER NOT NULL,
                text TEXT NOT NULL,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                status TEXT NOT NULL,
                error_message TEXT
            );

            CREATE TABLE IF NOT EXISTS vocabulary (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                term TEXT NOT NULL,
                alias TEXT NOT NULL DEFAULT '',
                category TEXT NOT NULL DEFAULT '',
                note TEXT NOT NULL DEFAULT '',
                enabled INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS style_prompts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                prompt TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS personalization_preferences (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                remove_trailing_period INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )?;
        Ok(())
    }

    pub fn get_config(&self) -> Result<AppConfig> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let mut config = Self::get_config_from_conn(&conn)?.unwrap_or_default();
        if config.hotkey.trim() == "F8" {
            config.hotkey = DEFAULT_HOTKEY.to_string();
            Self::save_config_from_conn(&conn, &config)?;
        }
        if let Some(provider) = Self::get_enabled_provider_from_conn(&conn)? {
            config.provider = provider.provider;
            config.base_url = provider.base_url;
            config.model = provider.model;
            config.api_key_ref = provider.api_key_ref;
        }
        Ok(config)
    }

    pub fn save_config(&self, config: &AppConfig) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        Self::save_config_from_conn(&conn, config)?;
        if config.has_complete_provider() {
            Self::upsert_provider_from_config(&conn, config)?;
        }
        Ok(())
    }

    pub fn list_provider_configs(&self) -> Result<Vec<ProviderConfig>> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        Self::list_provider_configs_from_conn(&conn)
    }

    pub fn save_provider_config(&self, provider: &ProviderConfig) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let enabled = provider.enabled || Self::provider_count_from_conn(&conn)? == 0;
        let id = if provider.id > 0 {
            conn.execute(
                r#"
                UPDATE provider_configs
                SET provider = ?1, base_url = ?2, model = ?3, api_key_ref = ?4, enabled = ?5
                WHERE id = ?6
                "#,
                params![
                    provider.provider,
                    provider.base_url,
                    provider.model,
                    provider.api_key_ref,
                    if enabled { 1 } else { 0 },
                    provider.id
                ],
            )?;
            provider.id
        } else {
            conn.execute(
                r#"
                INSERT INTO provider_configs (provider, base_url, model, api_key_ref, enabled)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    provider.provider,
                    provider.base_url,
                    provider.model,
                    provider.api_key_ref,
                    if enabled { 1 } else { 0 }
                ],
            )?;
            conn.last_insert_rowid()
        };
        if enabled {
            Self::select_provider_config_from_conn(&conn, id)?;
        } else {
            Self::ensure_enabled_provider_config(&conn)?;
        }
        Ok(())
    }

    pub fn select_provider_config(&self, id: i64) -> Result<AppConfig> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        Self::select_provider_config_from_conn(&conn, id)?;
        let mut config = Self::get_config_from_conn(&conn)?.unwrap_or_default();
        if let Some(provider) = Self::get_enabled_provider_from_conn(&conn)? {
            config.provider = provider.provider;
            config.base_url = provider.base_url;
            config.model = provider.model;
            config.api_key_ref = provider.api_key_ref;
        }
        Self::save_config_from_conn(&conn, &config)?;
        Ok(config)
    }

    pub fn delete_provider_config(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let was_enabled = conn
            .query_row(
                "SELECT enabled FROM provider_configs WHERE id = ?1",
                [id],
                |row| row.get::<_, i64>(0),
            )
            .map(|enabled| enabled == 1)
            .unwrap_or(false);
        conn.execute("DELETE FROM provider_configs WHERE id = ?1", [id])?;
        if was_enabled {
            conn.execute(
                r#"
                UPDATE provider_configs
                SET enabled = CASE
                    WHEN id = (SELECT id FROM provider_configs ORDER BY id DESC LIMIT 1) THEN 1
                    ELSE 0
                END
                "#,
                [],
            )?;
        }
        Self::normalize_enabled_provider_configs(&conn)?;
        if let Some(provider) = Self::get_enabled_provider_from_conn(&conn)? {
            let mut config = Self::get_config_from_conn(&conn)?.unwrap_or_default();
            config.provider = provider.provider;
            config.base_url = provider.base_url;
            config.model = provider.model;
            config.api_key_ref = provider.api_key_ref;
            Self::save_config_from_conn(&conn, &config)?;
        } else {
            let mut config = Self::get_config_from_conn(&conn)?.unwrap_or_default();
            config.provider.clear();
            config.base_url.clear();
            config.model.clear();
            config.api_key_ref.clear();
            Self::save_config_from_conn(&conn, &config)?;
        }
        Ok(())
    }

    pub fn insert_record(
        &self,
        text: &str,
        duration_seconds: u32,
        status: RecognitionStatus,
    ) -> Result<()> {
        self.insert_record_with_error(text, duration_seconds, status, None)
    }

    pub fn insert_record_with_error(
        &self,
        text: &str,
        duration_seconds: u32,
        status: RecognitionStatus,
        error_message: Option<&str>,
    ) -> Result<()> {
        let config = self.get_config()?;
        let status_text = match status {
            RecognitionStatus::Success => "success",
            RecognitionStatus::Failed => "failed",
            RecognitionStatus::Processing => "processing",
        };
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute(
            "INSERT INTO recognition_records (created_at, duration_seconds, text, provider, model, status, error_message) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                Utc::now().to_rfc3339(),
                duration_seconds,
                text,
                config.provider,
                config.model,
                status_text,
                error_message
            ],
        )?;
        Ok(())
    }

    pub fn list_records(&self, limit: usize) -> Result<Vec<RecognitionRecord>> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, created_at, duration_seconds, text, provider, model, status, error_message FROM recognition_records ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            let status_text: String = row.get(6)?;
            Ok(RecognitionRecord {
                id: row.get(0)?,
                created_at: row.get(1)?,
                duration_seconds: row.get::<_, i64>(2)? as u32,
                text: row.get(3)?,
                provider: row.get(4)?,
                model: row.get(5)?,
                status: match status_text.as_str() {
                    "success" => RecognitionStatus::Success,
                    "processing" => RecognitionStatus::Processing,
                    _ => RecognitionStatus::Failed,
                },
                error_message: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_vocabulary(&self) -> Result<Vec<VocabularyItem>> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, term, alias, category, note, enabled FROM vocabulary ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(VocabularyItem {
                id: row.get(0)?,
                term: row.get(1)?,
                alias: row.get(2)?,
                category: row.get(3)?,
                note: row.get(4)?,
                enabled: row.get::<_, i64>(5)? == 1,
            })
        })?;
        rows.collect()
    }

    pub fn add_vocabulary(&self, item: &VocabularyItem) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute(
            "INSERT INTO vocabulary (term, alias, category, note, enabled) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![item.term, item.alias, item.category, item.note, if item.enabled { 1 } else { 0 }],
        )?;
        Ok(())
    }

    pub fn add_vocabulary_terms(&self, terms: &[String]) -> Result<()> {
        let mut conn = self.conn.lock().expect("database mutex poisoned");
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO vocabulary (term, alias, category, note, enabled) VALUES (?1, '', '', '', 1)",
            )?;
            for term in terms {
                let normalized = term.trim();
                if !normalized.is_empty() {
                    stmt.execute([normalized])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_vocabulary(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute("DELETE FROM vocabulary WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn list_style_prompts(&self) -> Result<Vec<StylePrompt>> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let mut stmt =
            conn.prepare("SELECT id, name, prompt, enabled FROM style_prompts ORDER BY id DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(StylePrompt {
                id: row.get(0)?,
                name: row.get(1)?,
                prompt: row.get(2)?,
                enabled: row.get::<_, i64>(3)? == 1,
            })
        })?;
        rows.collect()
    }

    pub fn add_style_prompt(&self, item: &StylePrompt) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute(
            "INSERT INTO style_prompts (name, prompt, enabled) VALUES (?1, ?2, ?3)",
            params![item.name, item.prompt, if item.enabled { 1 } else { 0 }],
        )?;
        let id = conn.last_insert_rowid();
        if item.enabled {
            conn.execute("UPDATE style_prompts SET enabled = 0 WHERE id != ?1", [id])?;
        } else {
            Self::normalize_enabled_style_prompts(&conn)?;
        }
        Ok(())
    }

    pub fn update_style_prompt(&self, item: &StylePrompt) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let changed = conn.execute(
            "UPDATE style_prompts SET name = ?1, prompt = ?2, enabled = ?3 WHERE id = ?4",
            params![
                item.name,
                item.prompt,
                if item.enabled { 1 } else { 0 },
                item.id
            ],
        )?;
        if item.enabled && changed > 0 {
            conn.execute(
                "UPDATE style_prompts SET enabled = 0 WHERE id != ?1",
                [item.id],
            )?;
        } else {
            Self::normalize_enabled_style_prompts(&conn)?;
        }
        Ok(())
    }

    pub fn delete_style_prompt(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute("DELETE FROM style_prompts WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn get_personalization_preferences(&self) -> Result<PersonalizationPreferences> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        let result = conn.query_row(
            "SELECT remove_trailing_period FROM personalization_preferences WHERE id = 1",
            [],
            |row| {
                Ok(PersonalizationPreferences {
                    remove_trailing_period: row.get::<_, i64>(0)? == 1,
                })
            },
        );
        match result {
            Ok(preferences) => Ok(preferences),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(PersonalizationPreferences::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save_personalization_preferences(
        &self,
        preferences: &PersonalizationPreferences,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute(
            r#"
            INSERT INTO personalization_preferences (id, remove_trailing_period)
            VALUES (1, ?1)
            ON CONFLICT(id) DO UPDATE SET
                remove_trailing_period = excluded.remove_trailing_period
            "#,
            [if preferences.remove_trailing_period {
                1
            } else {
                0
            }],
        )?;
        Ok(())
    }

    fn normalize_enabled_style_prompts(conn: &Connection) -> Result<()> {
        conn.execute(
            r#"
            UPDATE style_prompts
            SET enabled = CASE
                WHEN id = (
                    SELECT id
                    FROM style_prompts
                    WHERE enabled = 1
                    ORDER BY id DESC
                    LIMIT 1
                ) THEN 1
                ELSE 0
            END
            WHERE enabled = 1
            "#,
            [],
        )?;
        Ok(())
    }

    fn get_config_from_conn(conn: &Connection) -> Result<Option<AppConfig>> {
        let result = conn.query_row(
            "SELECT provider, base_url, model, api_key_ref, hotkey FROM app_config WHERE id = 1",
            [],
            |row| {
                Ok(AppConfig {
                    provider: row.get(0)?,
                    base_url: row.get(1)?,
                    model: row.get(2)?,
                    api_key_ref: row.get(3)?,
                    hotkey: row.get(4)?,
                })
            },
        );
        match result {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn save_config_from_conn(conn: &Connection, config: &AppConfig) -> Result<()> {
        let hotkey = config.hotkey.trim();
        conn.execute(
            r#"
            INSERT INTO app_config (id, provider, base_url, model, api_key_ref, hotkey)
            VALUES (1, ?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
                provider = excluded.provider,
                base_url = excluded.base_url,
                model = excluded.model,
                api_key_ref = excluded.api_key_ref,
                hotkey = excluded.hotkey
            "#,
            params![
                config.provider,
                config.base_url,
                config.model,
                config.api_key_ref,
                hotkey
            ],
        )?;
        Ok(())
    }

    fn list_provider_configs_from_conn(conn: &Connection) -> Result<Vec<ProviderConfig>> {
        let mut stmt = conn.prepare(
            "SELECT id, provider, base_url, model, api_key_ref, enabled FROM provider_configs ORDER BY enabled DESC, id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ProviderConfig {
                id: row.get(0)?,
                provider: row.get(1)?,
                base_url: row.get(2)?,
                model: row.get(3)?,
                api_key_ref: row.get(4)?,
                enabled: row.get::<_, i64>(5)? == 1,
            })
        })?;
        rows.collect()
    }

    fn get_enabled_provider_from_conn(conn: &Connection) -> Result<Option<ProviderConfig>> {
        let result = conn.query_row(
            "SELECT id, provider, base_url, model, api_key_ref, enabled FROM provider_configs WHERE enabled = 1 ORDER BY id DESC LIMIT 1",
            [],
            |row| {
                Ok(ProviderConfig {
                    id: row.get(0)?,
                    provider: row.get(1)?,
                    base_url: row.get(2)?,
                    model: row.get(3)?,
                    api_key_ref: row.get(4)?,
                    enabled: row.get::<_, i64>(5)? == 1,
                })
            },
        );
        match result {
            Ok(provider) => Ok(Some(provider)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn provider_count_from_conn(conn: &Connection) -> Result<i64> {
        conn.query_row("SELECT COUNT(*) FROM provider_configs", [], |row| {
            row.get(0)
        })
    }

    fn upsert_provider_from_config(conn: &Connection, config: &AppConfig) -> Result<()> {
        let existing_id = conn
            .query_row(
                "SELECT id FROM provider_configs WHERE provider = ?1 AND model = ?2 LIMIT 1",
                params![config.provider, config.model],
                |row| row.get::<_, i64>(0),
            )
            .ok();
        if let Some(id) = existing_id {
            conn.execute(
                r#"
                UPDATE provider_configs
                SET base_url = ?1, api_key_ref = ?2, enabled = 1
                WHERE id = ?3
                "#,
                params![config.base_url, config.api_key_ref, id],
            )?;
            Self::select_provider_config_from_conn(conn, id)?;
        } else {
            conn.execute(
                r#"
                INSERT INTO provider_configs (provider, base_url, model, api_key_ref, enabled)
                VALUES (?1, ?2, ?3, ?4, 1)
                "#,
                params![
                    config.provider,
                    config.base_url,
                    config.model,
                    config.api_key_ref
                ],
            )?;
            Self::select_provider_config_from_conn(conn, conn.last_insert_rowid())?;
        }
        Ok(())
    }

    fn select_provider_config_from_conn(conn: &Connection, id: i64) -> Result<()> {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM provider_configs WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        conn.execute(
            r#"
            UPDATE provider_configs
            SET enabled = CASE WHEN id = ?1 THEN 1 ELSE 0 END
            "#,
            [id],
        )?;
        Ok(())
    }

    fn normalize_enabled_provider_configs(conn: &Connection) -> Result<()> {
        conn.execute(
            r#"
            UPDATE provider_configs
            SET enabled = CASE
                WHEN id = (
                    SELECT id
                    FROM provider_configs
                    WHERE enabled = 1
                    ORDER BY id DESC
                    LIMIT 1
                ) THEN 1
                ELSE 0
            END
            WHERE enabled = 1
            "#,
            [],
        )?;
        Ok(())
    }

    fn ensure_enabled_provider_config(conn: &Connection) -> Result<()> {
        let enabled_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM provider_configs WHERE enabled = 1",
            [],
            |row| row.get(0),
        )?;
        if enabled_count == 0 {
            conn.execute(
                r#"
                UPDATE provider_configs
                SET enabled = CASE
                    WHEN id = (SELECT id FROM provider_configs ORDER BY id DESC LIMIT 1) THEN 1
                    ELSE 0
                END
                "#,
                [],
            )?;
        } else {
            Self::normalize_enabled_provider_configs(conn)?;
        }
        Ok(())
    }
}

impl AppConfig {
    pub fn has_complete_provider(&self) -> bool {
        !self.provider.trim().is_empty()
            && !self.base_url.trim().is_empty()
            && !self.model.trim().is_empty()
            && !self.api_key_ref.trim().is_empty()
    }
}

impl ProviderConfig {
    pub fn has_complete_provider(&self) -> bool {
        !self.provider.trim().is_empty()
            && !self.base_url.trim().is_empty()
            && !self.model.trim().is_empty()
            && !self.api_key_ref.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_config_records_vocabulary_and_styles() {
        let db = AppDb::in_memory().unwrap();
        let config = AppConfig {
            provider: "Qwen".to_string(),
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            model: "qwen3.5-omni-plus".to_string(),
            api_key_ref: "credential-manager:qwen".to_string(),
            hotkey: "Alt+Space".to_string(),
        };

        db.save_config(&config).unwrap();
        db.insert_record("识别文本", 9, RecognitionStatus::Success)
            .unwrap();
        db.add_vocabulary(&VocabularyItem {
            id: 0,
            term: "Qwen".to_string(),
            alias: "通义千问".to_string(),
            category: "model".to_string(),
            note: String::new(),
            enabled: true,
        })
        .unwrap();
        db.add_style_prompt(&StylePrompt {
            id: 0,
            name: "书面语".to_string(),
            prompt: "整理为书面语".to_string(),
            enabled: true,
        })
        .unwrap();

        assert_eq!(db.get_config().unwrap().model, "qwen3.5-omni-plus");
        assert_eq!(db.list_records(10).unwrap()[0].text, "识别文本");
        assert_eq!(db.list_vocabulary().unwrap()[0].term, "Qwen");
        assert_eq!(db.list_style_prompts().unwrap()[0].name, "书面语");
    }

    #[test]
    fn manages_batch_vocabulary_and_style_prompt_changes() {
        let db = AppDb::in_memory().unwrap();

        db.add_vocabulary_terms(&["Qwen".to_string(), "  ".to_string(), "MiMo".to_string()])
            .unwrap();

        let vocabulary = db.list_vocabulary().unwrap();
        assert_eq!(vocabulary.len(), 2);
        assert_eq!(vocabulary[0].term, "MiMo");

        db.delete_vocabulary(vocabulary[0].id).unwrap();
        assert_eq!(db.list_vocabulary().unwrap().len(), 1);

        db.add_style_prompt(&StylePrompt {
            id: 0,
            name: "书面语".to_string(),
            prompt: "整理为书面语".to_string(),
            enabled: true,
        })
        .unwrap();

        let mut style = db.list_style_prompts().unwrap()[0].clone();
        style.name = "会议纪要".to_string();
        style.prompt = "整理为会议纪要".to_string();
        style.enabled = false;
        db.update_style_prompt(&style).unwrap();

        let updated = db.list_style_prompts().unwrap()[0].clone();
        assert_eq!(updated.name, "会议纪要");
        assert!(!updated.enabled);

        db.delete_style_prompt(updated.id).unwrap();
        assert!(db.list_style_prompts().unwrap().is_empty());
    }

    #[test]
    fn keeps_at_most_one_style_prompt_enabled() {
        let db = AppDb::in_memory().unwrap();

        db.add_style_prompt(&StylePrompt {
            id: 0,
            name: "书面语".to_string(),
            prompt: "整理为书面语".to_string(),
            enabled: true,
        })
        .unwrap();
        db.add_style_prompt(&StylePrompt {
            id: 0,
            name: "会议纪要".to_string(),
            prompt: "整理为会议纪要".to_string(),
            enabled: true,
        })
        .unwrap();

        let styles = db.list_style_prompts().unwrap();
        let enabled: Vec<_> = styles.iter().filter(|style| style.enabled).collect();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "会议纪要");

        let mut style = enabled[0].clone();
        style.enabled = false;
        db.update_style_prompt(&style).unwrap();

        let styles = db.list_style_prompts().unwrap();
        assert!(styles.iter().all(|style| !style.enabled));
    }

    #[test]
    fn stores_alt_hotkeys_without_rewriting_them() {
        let db = AppDb::in_memory().unwrap();
        let config = AppConfig {
            hotkey: "Alt".to_string(),
            ..Default::default()
        };

        db.save_config(&config).unwrap();

        assert_eq!(db.get_config().unwrap().hotkey, "Alt");
    }

    #[test]
    fn repairs_previous_f8_default_migration_to_alt() {
        let db = AppDb::in_memory().unwrap();
        let config = AppConfig {
            hotkey: "F8".to_string(),
            ..Default::default()
        };

        db.save_config(&config).unwrap();

        assert_eq!(db.get_config().unwrap().hotkey, "Alt");
    }
}
