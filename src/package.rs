use lazy_static::lazy_static;
use regex::Regex;
use std::cmp::{Eq, PartialEq};
use std::collections::{HashMap, HashSet};
use std::fs::read_to_string;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

fn extract_features(input: &str) -> Option<Vec<&str>> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r#"feature\s*=\s*"(?P<feature>((\w*)-*)*)""#).unwrap();
    }
    let mut res = Vec::new();
    for s in RE.find_iter(input) {
        res.push(s.as_str())
    }
    Some(res)
}

#[derive(Debug, Clone)]
pub enum Feature {
    UsedFeature {
        name: String,
        path: PathBuf,
        line_number: u64,
    },
    ExposedFeature {
        name: String,
    },
}

impl Hash for Feature {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name().hash(state);
    }
}

impl PartialEq for Feature {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl Eq for Feature {}

impl Feature {
    fn path(&self) -> Option<&Path> {
        match self {
            Self::UsedFeature { path, .. } => Some(path),
            Self::ExposedFeature { .. } => None,
        }
    }

    fn clickable_path(&self) -> Option<String> {
        match self {
            Self::UsedFeature {
                path, line_number, ..
            } => {
                let path_str = path.to_str().expect("toto SCOTT");
                let ln = line_number.to_string();
                let clickable_path = format!("{}:{}", path_str, ln);
                Some(clickable_path)
            }
            Self::ExposedFeature { .. } => None,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::UsedFeature { name, .. } => name,
            Self::ExposedFeature { name } => name,
        }
    }
}

fn cfg_features(json: serde_json::Value) -> Result<Vec<Feature>, &'static str> {
    let line = String::from(json["data"]["lines"]["text"].as_str().expect("SCOTT")); // error
    let features = match extract_features(&line) {
        Some(text) => text,
        None => return Err("get_rekt"),
    };
    let mut res = Vec::new();
    for feature_name in features {
        let line_number = json["data"]["line_number"]
            .as_u64()
            .expect("couldn't convert"); // error
        let path = Path::new(json["data"]["path"]["text"].as_str().unwrap()); // error
        res.push(Feature::UsedFeature {
            name: feature_name.to_string(),
            path: path.to_path_buf(),
            line_number,
        })
    }
    Ok(res)
}

#[derive(Debug)]
struct CrateInfo {
    path: PathBuf,
    // Set of all exposed features (represented as Strings).
    exposed_features: HashSet<Feature>,
    // Set of all used features, represented as Strings.
    used_features: HashSet<Feature>,
    // Set that represents the difference between the used features and the exposed features.
    hidden_features: HashSet<Feature>,
}

impl CrateInfo {
    fn new(path: &Path) -> Self {
        let path = path.to_path_buf();
        let exposed_features = HashSet::new();
        let used_features = HashSet::new();
        let hidden_features = HashSet::new();
        Self {
            path,
            exposed_features,
            used_features,
            hidden_features,
        }
    }

    fn add_used_features(&mut self, new_features: &HashSet<Feature>) {
        self.used_features = self.used_features.union(new_features).cloned().collect();
    }
}

fn run_rg_command(path: &Path) -> Result<Vec<u8>, String> {
    let output = Command::new("rg")
        // need to account for other feature = .. maybe regex?
        // need to account for multiline
        .args(&[
            // json output
            "--json",
            // use a regex expression to specify what we wish to capture
            "-e",
            r"feature\s*=",
            // only search for rust files
            "-trust",
            path.to_str().expect("SCOTT"),
        ]) // err
        .output()
        .expect("SCOTT"); // err
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// A mapping of Paths to Crates. Used to iterate over every crate in the directory.
#[derive(Debug)]
pub struct Package(HashMap<PathBuf, CrateInfo>);

impl Package {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    /// Find the Cargo.toml file associated to a path.
    pub fn find_associated_cargo(&self, mut path: &Path) -> Option<PathBuf> {
        loop {
            // Create a potential candidate by appending Cargo.toml
            let candidate = path.join("Cargo.toml");
            // Check whether this file exists.
            if self.0.contains_key(&candidate) || candidate.exists() {
                // It exists, so we've found the Cargo.toml that corresponds to the initial path.
                return Some(candidate);
            } else {
                // Bad candidate: loop back, this time using the parent directory as path.
                path = path.parent()?;
            }
        }
    }

    /// Finds the used features by ripgrep'ing the path, looking for occurences of cfg(feature)
    /// and groups those occurences by crates.
    pub fn find_used_features(&mut self, path: &Path) -> Result<(), String> {
        // Run the command and capture its output.
        let output = run_rg_command(path)?;

        let cow = String::from_utf8_lossy(&output);
        // Output is a bunch of json separated by \n, so we split them to iterate over them.
        let jsons = cow.split('\n');

        for line in jsons {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                // Keep only the "match" jsons.
                if json["type"] == "match" {
                    // use get?
                    let cfg = cfg_features(json);
                    match cfg {
                        Ok(v) => {
                            for feature in v {
                                self.add_feature(feature)
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
        }
        Ok(())
    }

    // scott rename
    /// Adds the feature to the mapping from paths to crates.
    pub fn add_feature(&mut self, feature: Feature) {
        let path = feature.path().expect("SCOTT");
        // The path to the parent directory
        let parent = path.parent().expect("failed to find cargo file");
        // Create a Cargo.toml path candidate: a Cargo file that would be in the same directory as the .rs file we just matched.
        let cargo_path = self.find_associated_cargo(&parent).expect("SCOTT");

        if !self.0.contains_key(&cargo_path) {
            let mut used_features_set = HashSet::new();
            // Populate the set with the list of used features.
            used_features_set.insert(feature);

            // Create a cargo entry, filled with the used vec.
            let mut cargo = CrateInfo::new(&cargo_path);
            cargo.add_used_features(&used_features_set);
            // Insert the Cargo entry in the path mapping.
            let _ = self.0.insert(cargo_path, cargo);
        } else {
            let toto = self.0.get_mut(&cargo_path).unwrap(); // toto SCOTT
            toto.used_features.insert(feature);
        }
    }

    pub fn find_exposed_features(&mut self) {
        // Iterate over every Cargo
        for v in self.0.values_mut() {
            // Load its content in a String
            let s = read_to_string(&v.path).expect("first");
            // Parse the Cargo into a TOML structure
            let toml = s.parse::<toml::Value>().unwrap();
            let table = match &toml.get("features") {
                Some(toml::Value::Table(table)) => Some(table),
                _ => None,
            };
            let mut exposed = HashSet::new();
            if let Some(table) = table {
                for (feature_name, _) in table.iter() {
                    exposed.insert(Feature::ExposedFeature {
                        name: feature_name.to_string(),
                    });
                }
            }
            v.exposed_features = exposed;
        }
    }

    pub fn find_hidden_features(&mut self) {
        for crate_ in self.0.values_mut() {
            let diff = crate_.used_features.difference(&crate_.exposed_features);
            let mut h = HashSet::new();
            for feature in diff {
                h.insert(feature.clone());
            }
            crate_.hidden_features = h;
        }
    }

    // todo pretty print
    pub fn display_hidden_features(&self) {
        println!("hidden");
        for cargo in self.0.values() {
            if !cargo.hidden_features.is_empty() {
                println!("path: {:?}", cargo.path);
            }
            for feature in cargo.hidden_features.iter() {
                println!(
                    "\t{}\t{}",
                    feature.name(),
                    feature.clickable_path().expect("should have path SCOTT")
                );
            }
        }
    }

    // todo pretty print
    pub fn display_exposed_features(&self) {
        println!("exposed");
        for cargo in self.0.values() {
            if !cargo.exposed_features.is_empty() {
                println!("path: {:?}", cargo.path);
            }
            for feature in cargo.exposed_features.iter() {
                println!(
                    "\t{}",
                    feature.name(),
                );
            }
        }
    }

    // todo pretty print
    pub fn display_used_features(&self) {
        println!("used");
        for cargo in self.0.values() {
            if !cargo.used_features.is_empty() {
                println!("path: {:?}", cargo.path);
            }
            for feature in cargo.used_features.iter() {
                println!(
                    "\t{}\t{}",
                    feature.name(),
                    feature.clickable_path().expect("should have path SCOTT")
                );
            }
        }
    }
}
