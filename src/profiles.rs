use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result, anyhow};
use directories::ProjectDirs;
use serde::Deserialize;

use crate::age::AgeThreshold;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub roots: Vec<PathBuf>,
    pub older_than: Option<AgeThreshold>,
}

#[derive(Debug, Clone, Default)]
pub struct Profiles {
    profiles: Vec<Profile>,
}

impl Profiles {
    pub fn load() -> Result<Self> {
        let Some(path) = config_path() else {
            return Ok(Self::default());
        };

        if !path.is_file() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Self::parse(&content)
            .with_context(|| format!("failed to parse profiles from {}", path.display()))
    }

    pub fn parse(content: &str) -> Result<Self> {
        let raw: RawProfiles = toml::from_str(content)?;
        let mut profiles = Vec::new();
        for (name, profile) in raw.profiles {
            let older_than = profile
                .older_than
                .as_deref()
                .map(AgeThreshold::from_str)
                .transpose()
                .map_err(|err| anyhow!("profile {name} has invalid older_than: {err}"))?;
            profiles.push(Profile {
                name,
                roots: profile
                    .roots
                    .into_iter()
                    .map(|root| PathBuf::from(expand_home(&root)))
                    .collect(),
                older_than,
            });
        }
        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self { profiles })
    }

    pub fn get(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|profile| profile.name == name)
    }

    pub fn all(&self) -> &[Profile] {
        &self.profiles
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

fn config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "nuked").map(|dirs| dirs.config_dir().join("profiles.toml"))
}

pub fn expand_home(raw: &str) -> String {
    if raw == "~" {
        return std::env::var("HOME").unwrap_or_else(|_| raw.to_string());
    }

    if let Some(rest) = raw.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }

    raw.to_string()
}

#[derive(Debug, Deserialize)]
struct RawProfiles {
    #[serde(default)]
    profiles: BTreeMap<String, RawProfile>,
}

#[derive(Debug, Deserialize)]
struct RawProfile {
    #[serde(default)]
    roots: Vec<String>,
    older_than: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::Profiles;

    #[test]
    fn parses_profiles() {
        let profiles = Profiles::parse(
            r#"
            [profiles.work]
            roots = ["~/Code", "/tmp/repos"]
            older_than = "7d"
            "#,
        )
        .unwrap();

        let profile = profiles.get("work").unwrap();
        assert_eq!(profile.roots.len(), 2);
        assert_eq!(profile.older_than.unwrap().as_days(), 7);
    }

    #[test]
    fn rejects_invalid_age() {
        let err = Profiles::parse(
            r#"
            [profiles.work]
            roots = ["/tmp/repos"]
            older_than = "later"
            "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("invalid older_than"));
    }
}
