use std::sync::Mutex;

use chrono::Utc;
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

use crate::models::{RecognitionRecord, RecognitionStatus, StylePrompt, VocabularyItem};

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

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: "MiMo".to_string(),
            base_url: "https://api.mimo-v2.com/v1".to_string(),
            model: "mimo-v2.5".to_string(),
            api_key_ref: "credential-manager:mimo".to_string(),
            hotkey: "Ctrl+Space".to_string(),
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
            "#,
        )?;
        Ok(())
    }

    pub fn get_config(&self) -> Result<AppConfig> {
        let conn = self.conn.lock().expect("database mutex poisoned");
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
            Ok(config) => Ok(config),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(AppConfig::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save_config(&self, config: &AppConfig) -> Result<()> {
        let conn = self.conn.lock().expect("database mutex poisoned");
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
                config.hotkey
            ],
        )?;
        Ok(())
    }

    pub fn insert_record(&self, text: &str, duration_seconds: u32, status: RecognitionStatus) -> Result<()> {
        let config = self.get_config()?;
        let status_text = match status {
            RecognitionStatus::Success => "success",
            RecognitionStatus::Failed => "failed",
            RecognitionStatus::Processing => "processing",
        };
        let conn = self.conn.lock().expect("database mutex poisoned");
        conn.execute(
            "INSERT INTO recognition_records (created_at, duration_seconds, text, provider, model, status, error_message) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            params![Utc::now().to_rfc3339(), duration_seconds, text, config.provider, config.model, status_text],
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
        let mut stmt = conn.prepare("SELECT id, term, alias, category, note, enabled FROM vocabulary ORDER BY id DESC")?;
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
        let mut stmt = conn.prepare("SELECT id, name, prompt, enabled FROM style_prompts ORDER BY id DESC")?;
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
            model: "qwen3.5-omni".to_string(),
            api_key_ref: "credential-manager:qwen".to_string(),
            hotkey: "Alt+Space".to_string(),
        };

        db.save_config(&config).unwrap();
        db.insert_record("识别文本", 9, RecognitionStatus::Success).unwrap();
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

        assert_eq!(db.get_config().unwrap().model, "qwen3.5-omni");
        assert_eq!(db.list_records(10).unwrap()[0].text, "识别文本");
        assert_eq!(db.list_vocabulary().unwrap()[0].term, "Qwen");
        assert_eq!(db.list_style_prompts().unwrap()[0].name, "书面语");
    }

    #[test]
    fn manages_batch_vocabulary_and_style_prompt_changes() {
        let db = AppDb::in_memory().unwrap();

        db.add_vocabulary_terms(&[
            "Qwen".to_string(),
            "  ".to_string(),
            "MiMo".to_string(),
        ])
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
}
