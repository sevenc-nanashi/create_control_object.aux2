use serde::{Deserialize, Serialize};
use std::io::Write as _;

const CONFIG_FILE_NAME: &str = "create_control_object.aux2.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IncompatibleEffectHandling {
    #[default]
    RemoveIncompatibleEffects,
    SkipToConvertibleObject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    pub(crate) incompatible_effect_handling: IncompatibleEffectHandling,
}

impl Settings {
    pub(crate) fn load() -> aviutl2::common::AnyResult<Self> {
        Self::load_from(&config_path()?)
    }

    pub(crate) fn save(&self) -> aviutl2::common::AnyResult<()> {
        self.save_to(&config_path()?)
    }

    fn load_from(path: &std::path::Path) -> aviutl2::common::AnyResult<Self> {
        let contents = match std::fs::read(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(error.into()),
        };
        Ok(serde_json::from_slice(&contents)?)
    }

    fn save_to(&self, path: &std::path::Path) -> aviutl2::common::AnyResult<()> {
        let parent = path
            .parent()
            .ok_or_else(|| aviutl2::anyhow::anyhow!("Config path must have a parent."))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&serde_json::to_vec_pretty(self)?)?;
        temporary.as_file_mut().sync_all()?;
        temporary.persist(path)?;
        Ok(())
    }
}

fn config_path() -> aviutl2::common::AnyResult<std::path::PathBuf> {
    let dll_path = process_path::get_dylib_path()
        .ok_or_else(|| aviutl2::anyhow::anyhow!("Could not determine the DLL path."))?;
    Ok(config_path_for_dll(&dll_path))
}

fn config_path_for_dll(dll_path: &std::path::Path) -> std::path::PathBuf {
    dll_path.with_file_name(CONFIG_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_removes_incompatible_effects() {
        assert_eq!(
            Settings::default().incompatible_effect_handling,
            IncompatibleEffectHandling::RemoveIncompatibleEffects
        );
    }

    #[test]
    fn settings_round_trip_json() {
        for incompatible_effect_handling in [
            IncompatibleEffectHandling::RemoveIncompatibleEffects,
            IncompatibleEffectHandling::SkipToConvertibleObject,
        ] {
            let settings = Settings {
                incompatible_effect_handling,
            };
            let encoded = serde_json::to_vec(&settings).unwrap();
            let decoded = serde_json::from_slice::<Settings>(&encoded).unwrap();
            assert_eq!(decoded, settings);
        }
    }

    #[test]
    fn missing_file_loads_default_settings() {
        let directory = tempfile::tempdir().unwrap();
        let settings = Settings::load_from(&directory.path().join("missing.json")).unwrap();
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn settings_round_trip_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.json");
        Settings::default().save_to(&path).unwrap();
        let settings = Settings {
            incompatible_effect_handling: IncompatibleEffectHandling::SkipToConvertibleObject,
        };
        settings.save_to(&path).unwrap();
        assert_eq!(Settings::load_from(&path).unwrap(), settings);
    }

    #[test]
    fn invalid_settings_are_rejected() {
        assert!(serde_json::from_str::<Settings>("not JSON").is_err());
        assert!(
            serde_json::from_str::<Settings>(r#"{"incompatible_effect_handling":"unknown"}"#)
                .is_err()
        );
        assert!(
            serde_json::from_str::<Settings>(
                r#"{"incompatible_effect_handling":"remove_incompatible_effects","unknown":true}"#
            )
            .is_err()
        );
    }

    #[test]
    fn config_path_is_next_to_dll() {
        assert_eq!(
            config_path_for_dll(std::path::Path::new(
                r"C:\Plugin\create_control_object.aux2"
            )),
            std::path::Path::new(r"C:\Plugin\create_control_object.aux2.json")
        );
    }
}
