//! The mods turned on (`horizon-mods.txt`), all off by default.
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub const DIR: &str = "mods";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Installed {
    pub id: String,
    pub title: String,
    pub description: String,
    pub problem: Option<String>,
    pub dev: bool,
    pub has_settings: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModList {
    pub installed: Vec<Installed>,
    pub enabled: HashSet<String>,
    pub keys: HashMap<String, HashMap<String, String>>,
}

impl ModList {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("horizon-mods.txt");
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text),
            Err(_) => Self::default(),
        }
    }
    pub fn save(&self, dir: &Path) {
        let path = dir.join("horizon-mods.txt");
        let _ = std::fs::write(path, self.serialize());
    }
    pub fn parse(text: &str) -> Self {
        let mut list = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((id, on)) = line.split_once(':') {
                if on.trim() == "true" {
                    list.enabled.insert(id.trim().to_owned());
                }
            }
        }
        list
    }
    pub fn serialize(&self) -> String {
        let mut ids: Vec<_> = self.enabled.iter().cloned().collect();
        ids.sort();
        ids.into_iter().map(|id| format!("mod.{id}:true\n")).collect()
    }
    pub fn is_on(&self, i: usize) -> bool {
        self.installed
            .get(i)
            .is_some_and(|m| self.enabled.contains(&m.id))
    }
    pub fn toggle(&mut self, i: usize) {
        if let Some(m) = self.installed.get(i) {
            if !self.enabled.remove(&m.id) {
                self.enabled.insert(m.id.clone());
            }
        }
    }
}