//! Skills: named instruction bundles loaded from `SKILL.md` files.
//!
//! Instructions are content. Loading a skill never executes code or grants
//! tools.
use crate::agent::tool::AgentError;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MAX_SKILLS: usize = 64;
pub const MAX_SKILL_FILE_BYTES: u64 = 70_000;
pub const MAX_DESCRIPTION_BYTES: usize = 1024;
pub const MAX_INSTRUCTIONS_BYTES: usize = 65_536;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSkill {
    pub name: String,
    pub description: String,
    pub instructions: String,
}

impl AgentSkill {
    pub fn new(name: impl Into<String>, description: impl Into<String>, instructions: impl Into<String>) -> Result<Self, AgentError> {
        let skill = Self {
            name: name.into(),
            description: description.into(),
            instructions: instructions.into(),
        };
        let valid_name = {
            let name = &skill.name;
            let mut characters = name.chars();
            match characters.next() {
                Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
                _ => return Err(invalid()),
            }
            let rest: String = characters.collect();
            rest.chars().count() <= 63
                && rest
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        };
        if !valid_name
            || skill.description.is_empty()
            || skill.description.len() > MAX_DESCRIPTION_BYTES
            || skill.instructions.is_empty()
            || skill.instructions.len() > MAX_INSTRUCTIONS_BYTES
        {
            return Err(invalid());
        }
        Ok(skill)
    }
}

fn invalid() -> AgentError {
    AgentError::InvalidConfiguration("Invalid skill name, description or instructions.".into())
}

/// A bounded, ordered set of skills.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentSkillLibrary {
    pub skills: Vec<AgentSkill>,
}

impl AgentSkillLibrary {
    pub fn new(skills: Vec<AgentSkill>) -> Result<Self, AgentError> {
        let mut names = std::collections::BTreeSet::new();
        if skills.len() > MAX_SKILLS || !skills.iter().all(|skill| names.insert(skill.name.clone())) {
            return Err(AgentError::InvalidConfiguration(
                "Skills must have unique names; at most 64 are supported.".into(),
            ));
        }
        // Re-validate decoded values as well as values built here.
        let skills = skills
            .into_iter()
            .map(|skill| AgentSkill::new(skill.name, skill.description, skill.instructions))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { skills })
    }

    pub fn read(&self, name: &str) -> Result<&AgentSkill, AgentError> {
        self.skills
            .iter()
            .find(|skill| skill.name == name)
            .ok_or_else(|| AgentError::InvalidConfiguration(format!("Unknown skill: {name}")))
    }

    pub fn instructions(&self, names: &[String]) -> Result<String, AgentError> {
        if names.iter().collect::<std::collections::BTreeSet<_>>().len() != names.len() {
            return Err(AgentError::InvalidConfiguration("Duplicate enabled skill.".into()));
        }
        names
            .iter()
            .map(|name| {
                let skill = self.read(name)?;
                Ok(format!("Skill: {}\n{}", skill.name, skill.instructions))
            })
            .collect::<Result<Vec<_>, AgentError>>()
            .map(|sections| sections.join("\n\n"))
    }

    /// Read immediate child directories containing `SKILL.md`. Only scalar
    /// `name` and `description` frontmatter is accepted; referenced resources
    /// are not read.
    pub fn load(directory: &Path) -> Result<Self, AgentError> {
        let root = canonical(directory)?;
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&root).map_err(|_| invalid())? {
            let Ok(entry) = entry else { continue };
            let name = entry.file_name();
            // `skipsHiddenFiles`
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            let Ok(metadata) = entry.metadata() else { continue };
            if !metadata.is_dir() {
                continue;
            }
            entries.push(entry.path());
        }
        entries.sort_by(|left, right| {
            left.file_name()
                .unwrap_or_default()
                .cmp(right.file_name().unwrap_or_default())
        });
        let mut skills = Vec::new();
        for path in entries {
            let file = match canonical(&path.join("SKILL.md")) {
                Ok(file) => file,
                Err(_) => continue,
            };
            if !file.exists() {
                continue;
            }
            if skills.len() >= MAX_SKILLS {
                return Err(AgentError::InvalidConfiguration("Skill library exceeds 64 entries.".into()));
            }
            if !file.starts_with(&root) || file == root {
                return Err(AgentError::InvalidConfiguration(
                    "Skill file escapes the configured directory.".into(),
                ));
            }
            let metadata = std::fs::metadata(&file).map_err(|_| invalid())?;
            if !metadata.is_file() || metadata.len() > MAX_SKILL_FILE_BYTES {
                return Err(AgentError::InvalidConfiguration(
                    "Skill must be a regular Markdown file of at most 70 KB.".into(),
                ));
            }
            let text = std::fs::read_to_string(&file).map_err(|_| invalid())?;
            skills.push(parse_skill(&text)?);
        }
        Self::new(skills)
    }
}

fn canonical(path: &Path) -> Result<std::path::PathBuf, AgentError> {
    std::fs::canonicalize(path).map_err(|_| AgentError::InvalidConfiguration(
        "Skill directory is not readable.".into(),
    ))
}

fn parse_skill(text: &str) -> Result<AgentSkill, AgentError> {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return Err(AgentError::InvalidConfiguration(
            "SKILL.md requires name and description frontmatter.".into(),
        ));
    }
    let mut fields: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut body_started = false;
    let mut body = Vec::new();
    for line in lines {
        if !body_started {
            if line == "---" {
                body_started = true;
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            let Some(colon) = line.find(':') else {
                return Err(AgentError::InvalidConfiguration(
                    "Skill frontmatter supports scalar name and description fields.".into(),
                ));
            };
            let key = line[..colon].trim().to_string();
            let mut value = line[colon + 1..].trim().to_string();
            if value.len() >= 2
                && ((value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\'')))
            {
                value = value[1..value.len() - 1].to_string();
            }
            let supported = key == "name" || key == "description";
            if !supported || fields.contains_key(&key) || value == "|" || value == ">" {
                return Err(AgentError::InvalidConfiguration(
                    "Unsupported or duplicate skill frontmatter field.".into(),
                ));
            }
            fields.insert(key, value);
        } else {
            body.push(line);
        }
    }
    if !body_started {
        return Err(AgentError::InvalidConfiguration(
            "SKILL.md requires name and description frontmatter.".into(),
        ));
    }
    AgentSkill::new(
        fields.remove("name").unwrap_or_default(),
        fields.remove("description").unwrap_or_default(),
        body.join("\n").trim().to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_is_scalar_only() {
        let skill = parse_skill("---\nname: ui-diagnostics\ndescription: \"Investigate UI\"\n---\nDo the thing.\n").unwrap();
        assert_eq!(skill.name, "ui-diagnostics");
        assert_eq!(skill.description, "Investigate UI");
        assert_eq!(skill.instructions, "Do the thing.");
    }

    #[test]
    fn block_scalars_and_unknown_keys_are_rejected() {
        assert!(parse_skill("---\nname: a\nbody: |\n---\nx\n").is_err());
        assert!(parse_skill("---\nname: a\nname: b\ndescription: d\n---\nx\n").is_err());
        assert!(parse_skill("no frontmatter\n").is_err());
    }

    #[test]
    fn duplicate_enabled_skills_are_rejected() {
        let library = AgentSkillLibrary::new(vec![
            AgentSkill::new("a", "d", "i").unwrap(),
            AgentSkill::new("b", "d", "i").unwrap(),
        ])
        .unwrap();
        assert!(library.instructions(&["a".into(), "a".into()]).is_err());
        assert!(library.instructions(&["a".into(), "b".into()]).is_ok());
        assert!(library.read("missing").is_err());
    }
}
