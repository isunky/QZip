#![forbid(unsafe_code)]

//! Cross-platform contracts for settings and shell integration.
//!
//! OS-specific registration is intentionally kept out of this crate so the
//! desktop host can persist and expose the same safe contracts on every
//! platform.

use archive_core::{ArchiveFormat, CompressionProfile, ConflictPolicy};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SETTINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
    System,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccentTheme {
    #[default]
    Mint,
    Ocean,
    Lavender,
    Amber,
    Coral,
    CyanSlate,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UiScale {
    Scale90,
    #[default]
    Scale100,
    Scale110,
    Scale125,
}

impl UiScale {
    pub const fn factor(self) -> f64 {
        match self {
            Self::Scale90 => 0.9,
            Self::Scale100 => 1.0,
            Self::Scale110 => 1.1,
            Self::Scale125 => 1.25,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ListDensity {
    #[default]
    Comfortable,
    Compact,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LanguagePreference {
    System,
    #[default]
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en-US")]
    EnUs,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub schema_version: u32,
    #[serde(default)]
    pub language: LanguagePreference,
    pub theme_mode: ThemeMode,
    pub accent_theme: AccentTheme,
    pub ui_scale: UiScale,
    pub list_density: ListDensity,
    pub reduce_motion: bool,
    pub default_format: ArchiveFormat,
    pub compression_profile: CompressionProfile,
    pub conflict_policy: ConflictPolicy,
    pub extract_to_named_folder: bool,
    pub avoid_duplicate_root_folder: bool,
    pub open_folder_after_extract: bool,
    pub test_after_create: bool,
    pub task_notifications_enabled: bool,
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub check_updates_on_startup: bool,
    /// Deliberately fixed off: QZip has no telemetry collection path.
    pub telemetry_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            language: LanguagePreference::ZhCn,
            theme_mode: ThemeMode::Light,
            accent_theme: AccentTheme::Mint,
            ui_scale: UiScale::Scale100,
            list_density: ListDensity::Comfortable,
            reduce_motion: false,
            default_format: ArchiveFormat::SevenZip,
            compression_profile: CompressionProfile::Balanced,
            conflict_policy: ConflictPolicy::Rename,
            extract_to_named_folder: true,
            avoid_duplicate_root_folder: true,
            open_folder_after_extract: false,
            test_after_create: true,
            task_notifications_enabled: false,
            notify_on_success: true,
            notify_on_failure: true,
            check_updates_on_startup: false,
            telemetry_enabled: false,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsPatch {
    pub language: Option<LanguagePreference>,
    pub theme_mode: Option<ThemeMode>,
    pub accent_theme: Option<AccentTheme>,
    pub ui_scale: Option<UiScale>,
    pub list_density: Option<ListDensity>,
    pub reduce_motion: Option<bool>,
    pub default_format: Option<ArchiveFormat>,
    pub compression_profile: Option<CompressionProfile>,
    pub conflict_policy: Option<ConflictPolicy>,
    pub extract_to_named_folder: Option<bool>,
    pub avoid_duplicate_root_folder: Option<bool>,
    pub open_folder_after_extract: Option<bool>,
    pub test_after_create: Option<bool>,
    pub task_notifications_enabled: Option<bool>,
    pub notify_on_success: Option<bool>,
    pub notify_on_failure: Option<bool>,
    pub check_updates_on_startup: Option<bool>,
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("default archive format must be writable")]
    ReadOnlyDefaultFormat,
}

impl AppSettings {
    pub fn migrated(value: serde_json::Value) -> Self {
        // Unknown, future, or corrupted values recover to a privacy-preserving
        // default. This keeps startup deterministic instead of exposing a raw
        // store document to the webview.
        let mut settings = serde_json::from_value::<Self>(value).unwrap_or_default();
        if settings.schema_version != SETTINGS_SCHEMA_VERSION {
            settings = Self::default();
        }
        settings.telemetry_enabled = false;
        if !settings.default_format.is_writable() {
            settings.default_format = ArchiveFormat::SevenZip;
        }
        settings
    }

    pub fn apply(&mut self, patch: AppSettingsPatch) -> Result<(), SettingsError> {
        if let Some(value) = patch.language {
            self.language = value;
        }
        if let Some(value) = patch.default_format {
            if !value.is_writable() {
                return Err(SettingsError::ReadOnlyDefaultFormat);
            }
            self.default_format = value;
        }
        if let Some(value) = patch.theme_mode {
            self.theme_mode = value;
        }
        if let Some(value) = patch.accent_theme {
            self.accent_theme = value;
        }
        if let Some(value) = patch.ui_scale {
            self.ui_scale = value;
        }
        if let Some(value) = patch.list_density {
            self.list_density = value;
        }
        if let Some(value) = patch.reduce_motion {
            self.reduce_motion = value;
        }
        if let Some(value) = patch.compression_profile {
            self.compression_profile = value;
        }
        if let Some(value) = patch.conflict_policy {
            self.conflict_policy = value;
        }
        if let Some(value) = patch.extract_to_named_folder {
            self.extract_to_named_folder = value;
        }
        if let Some(value) = patch.avoid_duplicate_root_folder {
            self.avoid_duplicate_root_folder = value;
        }
        if let Some(value) = patch.open_folder_after_extract {
            self.open_folder_after_extract = value;
        }
        if let Some(value) = patch.test_after_create {
            self.test_after_create = value;
        }
        if let Some(value) = patch.task_notifications_enabled {
            self.task_notifications_enabled = value;
        }
        if let Some(value) = patch.notify_on_success {
            self.notify_on_success = value;
        }
        if let Some(value) = patch.notify_on_failure {
            self.notify_on_failure = value;
        }
        if let Some(value) = patch.check_updates_on_startup {
            self.check_updates_on_startup = value;
        }
        self.telemetry_enabled = false;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatus {
    pub platform: String,
    pub file_associations_declared: bool,
    pub modern_context_menu_available: bool,
    pub modern_context_menu_registered: bool,
    pub updater_configured: bool,
    pub distribution: String,
    pub app_version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchKind {
    Open,
    CompressSevenZip,
    CompressZip,
    ExtractHere,
    ExtractNamed,
    MoreOptions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub kind: LaunchKind,
    pub paths: Vec<std::path::PathBuf>,
    pub source: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_safe_by_default() {
        let settings = AppSettings::default();
        assert!(!settings.telemetry_enabled);
        assert_eq!(settings.conflict_policy, ConflictPolicy::Rename);
    }

    #[test]
    fn cannot_make_a_read_only_format_default() {
        let mut settings = AppSettings::default();
        assert!(
            settings
                .apply(AppSettingsPatch {
                    default_format: Some(ArchiveFormat::Rar),
                    ..Default::default()
                })
                .is_err()
        );
    }

    #[test]
    fn corrupted_value_recovers_to_default() {
        assert_eq!(
            AppSettings::migrated(serde_json::json!({ "schemaVersion": 99 })),
            AppSettings::default()
        );
    }

    #[test]
    fn serialized_settings_have_no_password_field() {
        let document = serde_json::to_string(&AppSettings::default()).unwrap();
        assert!(!document.to_ascii_lowercase().contains("password"));
    }

    #[test]
    fn language_preference_serializes_for_the_frontend() {
        let mut settings = AppSettings::default();
        settings.language = LanguagePreference::System;
        let document = serde_json::to_value(settings).unwrap();
        assert_eq!(document["language"], "system");
    }

    #[test]
    fn settings_without_a_language_keep_existing_preferences() {
        let mut document = serde_json::to_value(AppSettings::default()).unwrap();
        document.as_object_mut().unwrap().remove("language");
        document["themeMode"] = serde_json::json!("dark");
        let migrated = AppSettings::migrated(document);
        assert_eq!(migrated.language, LanguagePreference::ZhCn);
        assert_eq!(migrated.theme_mode, ThemeMode::Dark);
    }
}
