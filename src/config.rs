use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use crate::ui::{
    columns::{JobColumn, SortColumn, SortOrder},
    theme::Theme,
};

const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preferences {
    pub theme: Theme,
    pub refresh_interval: u64,
    pub selected_columns: Vec<JobColumn>,
    pub sort_columns: Vec<SortColumn>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::SakuraLight,
            refresh_interval: 10,
            selected_columns: JobColumn::defaults(),
            sort_columns: vec![SortColumn {
                column: JobColumn::Id,
                order: SortOrder::Ascending,
            }],
        }
    }
}

impl Preferences {
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        fs::read_to_string(path)
            .ok()
            .and_then(|content| Self::from_toml(&content))
            .unwrap_or_default()
    }

    pub fn save(&self) -> io::Result<()> {
        let path = config_path().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "no user configuration directory")
        })?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_toml())
    }

    fn to_toml(&self) -> String {
        let columns = self
            .selected_columns
            .iter()
            .map(|column| format!("\"{}\"", column.id()))
            .collect::<Vec<_>>()
            .join(", ");
        let sorts = self
            .sort_columns
            .iter()
            .map(|sort| format!("\"{}:{}\"", sort.column.id(), sort.order.id()))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "version = {CONFIG_VERSION}\ntheme = \"{}\"\nrefresh_interval = {}\ncolumns = [{columns}]\nsorts = [{sorts}]\n",
            self.theme.id(),
            self.refresh_interval
        )
    }

    fn from_toml(content: &str) -> Option<Self> {
        let version = value(content, "version")?.parse::<u32>().ok()?;
        if version != CONFIG_VERSION {
            return None;
        }

        let mut preferences = Self::default();
        preferences.theme = value(content, "theme")
            .and_then(|item| Theme::from_id(unquote(item)))
            .unwrap_or(preferences.theme);
        preferences.refresh_interval = value(content, "refresh_interval")
            .and_then(|item| item.parse::<u64>().ok())
            .filter(|seconds| (1..=3600).contains(seconds))
            .unwrap_or(preferences.refresh_interval);

        let columns = value(content, "columns")
            .map(array_values)
            .unwrap_or_default()
            .into_iter()
            .filter_map(JobColumn::from_id)
            .collect::<Vec<_>>();
        if !columns.is_empty() {
            preferences.selected_columns = columns;
        }

        let sorts = value(content, "sorts")
            .map(array_values)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                let (column, order) = item.split_once(':')?;
                Some(SortColumn {
                    column: JobColumn::from_id(column)?,
                    order: SortOrder::from_id(order)?,
                })
            })
            .filter(|sort| preferences.selected_columns.contains(&sort.column))
            .collect::<Vec<_>>();
        preferences.sort_columns = sorts;

        Some(preferences)
    }
}

fn value<'a>(content: &'a str, key: &str) -> Option<&'a str> {
    content.lines().find_map(|line| {
        let line = line.split('#').next()?.trim();
        let (candidate, value) = line.split_once('=')?;
        (candidate.trim() == key).then_some(value.trim())
    })
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn array_values(value: &str) -> Vec<&str> {
    value
        .trim()
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .map(unquote)
        .filter(|value| !value.is_empty())
        .collect()
}

fn config_path() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        env::var_os("APPDATA").map(PathBuf::from)
    } else if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        Some(PathBuf::from(path))
    } else {
        env::var_os("HOME").map(|home| Path::new(&home).join(".config"))
    }
    .map(|root| root.join("slurmer").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_round_trip() {
        let preferences = Preferences {
            theme: Theme::Classic,
            refresh_interval: 30,
            selected_columns: vec![JobColumn::Id, JobColumn::Name],
            sort_columns: vec![SortColumn {
                column: JobColumn::Name,
                order: SortOrder::Descending,
            }],
        };
        assert_eq!(
            Preferences::from_toml(&preferences.to_toml()),
            Some(preferences)
        );
    }

    #[test]
    fn invalid_values_fall_back_safely() {
        let parsed = Preferences::from_toml(
            "version = 1\ntheme = \"dark-neon\"\nrefresh_interval = 0\ncolumns = [\"missing\"]\nsorts = [\"missing:up\"]\n",
        )
        .unwrap();
        assert_eq!(parsed.refresh_interval, 10);
        assert_eq!(parsed.selected_columns, JobColumn::defaults());
        assert!(parsed.sort_columns.is_empty());
    }

    #[test]
    fn existing_dark_neon_preference_is_preserved() {
        let parsed = Preferences::from_toml(
            "version = 1\ntheme = \"dark-neon\"\nrefresh_interval = 10\ncolumns = [\"id\"]\nsorts = []\n",
        )
        .unwrap();
        assert_eq!(parsed.theme, Theme::DarkNeon);
    }
}
