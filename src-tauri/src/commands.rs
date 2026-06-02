use serde::Serialize;

use crate::{
    db::{AppConfig, AppDb},
    models::{RecognitionRecord, RecognitionStatus, StylePrompt, VocabularyItem},
    platform::{current_platform_status, PlatformStatus},
    prompt::build_prompt_context,
    stats::{aggregate_usage_stats, UsageStats},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardData {
    pub stats: UsageStats,
    pub records: Vec<RecognitionRecord>,
    pub platform: PlatformStatus,
}

pub fn dashboard_data(db: &AppDb) -> Result<DashboardData, String> {
    let records = db.list_records(50).map_err(|error| error.to_string())?;
    Ok(DashboardData {
        stats: aggregate_usage_stats(&records),
        records,
        platform: current_platform_status(),
    })
}

pub fn get_config_data(db: &AppDb) -> Result<AppConfig, String> {
    db.get_config().map_err(|error| error.to_string())
}

pub fn save_config_data(db: &AppDb, config: AppConfig) -> Result<AppConfig, String> {
    db.save_config(&config).map_err(|error| error.to_string())?;
    Ok(config)
}

pub fn list_records_data(db: &AppDb) -> Result<Vec<RecognitionRecord>, String> {
    db.list_records(200).map_err(|error| error.to_string())
}

pub fn list_vocabulary_data(db: &AppDb) -> Result<Vec<VocabularyItem>, String> {
    db.list_vocabulary().map_err(|error| error.to_string())
}

pub fn add_vocabulary_data(db: &AppDb, item: VocabularyItem) -> Result<Vec<VocabularyItem>, String> {
    db.add_vocabulary(&item).map_err(|error| error.to_string())?;
    list_vocabulary_data(db)
}

pub fn list_style_prompts_data(db: &AppDb) -> Result<Vec<StylePrompt>, String> {
    db.list_style_prompts().map_err(|error| error.to_string())
}

pub fn add_style_prompt_data(db: &AppDb, item: StylePrompt) -> Result<Vec<StylePrompt>, String> {
    db.add_style_prompt(&item).map_err(|error| error.to_string())?;
    list_style_prompts_data(db)
}

pub fn simulate_recognition_data(db: &AppDb) -> Result<RecognitionRecord, String> {
    let vocabulary = db.list_vocabulary().map_err(|error| error.to_string())?;
    let styles = db.list_style_prompts().map_err(|error| error.to_string())?;
    let records = db.list_records(10).map_err(|error| error.to_string())?;
    let context = build_prompt_context(&vocabulary, &styles, &records);
    let text = if context.contains("书面语") {
        "这是一次模拟语音识别结果，已按照书面语风格整理。"
    } else {
        "这是一次模拟语音识别结果。"
    };
    db.insert_record(text, 6, RecognitionStatus::Success)
        .map_err(|error| error.to_string())?;
    db.list_records(1)
        .map_err(|error| error.to_string())?
        .into_iter()
        .next()
        .ok_or_else(|| "failed to load simulated record".to_string())
}

#[cfg(feature = "desktop")]
mod tauri_commands {
    use tauri::State;

    use super::*;

    #[tauri::command]
    pub fn get_dashboard(db: State<'_, AppDb>) -> Result<DashboardData, String> {
        dashboard_data(&db)
    }

    #[tauri::command]
    pub fn get_config(db: State<'_, AppDb>) -> Result<AppConfig, String> {
        get_config_data(&db)
    }

    #[tauri::command]
    pub fn save_config(db: State<'_, AppDb>, config: AppConfig) -> Result<AppConfig, String> {
        save_config_data(&db, config)
    }

    #[tauri::command]
    pub fn list_records(db: State<'_, AppDb>) -> Result<Vec<RecognitionRecord>, String> {
        list_records_data(&db)
    }

    #[tauri::command]
    pub fn list_vocabulary(db: State<'_, AppDb>) -> Result<Vec<VocabularyItem>, String> {
        list_vocabulary_data(&db)
    }

    #[tauri::command]
    pub fn add_vocabulary(db: State<'_, AppDb>, item: VocabularyItem) -> Result<Vec<VocabularyItem>, String> {
        add_vocabulary_data(&db, item)
    }

    #[tauri::command]
    pub fn list_style_prompts(db: State<'_, AppDb>) -> Result<Vec<StylePrompt>, String> {
        list_style_prompts_data(&db)
    }

    #[tauri::command]
    pub fn add_style_prompt(db: State<'_, AppDb>, item: StylePrompt) -> Result<Vec<StylePrompt>, String> {
        add_style_prompt_data(&db, item)
    }

    #[tauri::command]
    pub fn simulate_recognition(db: State<'_, AppDb>) -> Result<RecognitionRecord, String> {
        simulate_recognition_data(&db)
    }

    pub fn handlers<R: tauri::Runtime>() -> Box<dyn Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static> {
        Box::new(tauri::generate_handler![
            get_dashboard,
            get_config,
            save_config,
            list_records,
            list_vocabulary,
            add_vocabulary,
            list_style_prompts,
            add_style_prompt,
            simulate_recognition
        ])
    }
}

#[cfg(feature = "desktop")]
pub use tauri_commands::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulated_recognition_persists_a_success_record() {
        let db = AppDb::in_memory().unwrap();

        let record = simulate_recognition_data(&db).unwrap();
        let dashboard = dashboard_data(&db).unwrap();

        assert_eq!(record.status, RecognitionStatus::Success);
        assert_eq!(dashboard.stats.total_records, 1);
        assert_eq!(dashboard.records[0].text, record.text);
    }
}
