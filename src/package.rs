use lazy_static::lazy_static;
use regex::Regex;
use std::cmp::{Eq, PartialEq};
use std::collections::{HashMap, HashSet};
use std::fs::read_to_string;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Extracts the features from a given string and collects them into a Vector.
/// e.g `"#[cfg(features = "foo", features= "bar")]"` -> `vec!["foo", "bar"]`
fn extract_features(input: &str) -> Vec<&str> {
    // Using lazy_static here to avoid having to compile this regex everytime.
    lazy_static! {
        static ref RE: Regex =
            Regex::new(r#"feature\s*=\s*"(?P<feature>((\w*)-*)*)""#).expect("Invalid regex");
    }
    RE.captures_iter(input)
        // For each match, extract the "feature" group which we just captured.
        .map(|c| match c.name("feature") {
            Some(val) => val.as_str(),
            None => unreachable!("SCOTT"),
        })
        .collect()
}

/// Struct that represents a feature.
#[derive(Debug, Clone)]
pub enum Feature {
    // A feature that is used inside the code. The path and line_number are kept
    // so that we can produce a clickable link when we print it.
    UsedFeature {
        name: String,
        path: PathBuf,
        line_number: u64,
    },
    // A feature that is exposed by a Cargo.toml.
    ExposedFeature {
        name: String,
    },
}

// We implement Hash for Feature because we want to objects to be identical if they share the same name.
// This avoids having tons of different lines written out if they all share the same feature in the same crate.
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
    /// Returns the path to the file, if it exists.
    fn path(&self) -> Option<&Path> {
        match self {
            Self::UsedFeature { path, .. } => Some(path),
            Self::ExposedFeature { .. } => None,
        }
    }

    /// Returns a clinkable link to the feature inside the code.
    fn clickable_path(&self) -> Option<String> {
        match self {
            Self::UsedFeature {
                path, line_number, ..
            } => {
                let clickable_path = format!("{:?}:{}", path, line_number);
                Some(clickable_path)
            }
            Self::ExposedFeature { .. } => None,
        }
    }

    /// Returns the name of the feature.
    fn name(&self) -> &str {
        match self {
            Self::UsedFeature { name, .. } | Self::ExposedFeature { name } => name,
        }
    }
}

/// Extracts the features from a json object.
fn cfg_features(json: &serde_json::Value) -> Result<Vec<Feature>, &'static str> {
    let line = String::from(json["data"]["lines"]["text"].as_str().expect("SCOTT")); // error
    let feature_names = extract_features(&line);
    let mut features = Vec::new();
    for feature_name in feature_names {
        let line_number = json["data"]["line_number"]
            .as_u64()
            .expect("couldn't convert"); // error
        let path = Path::new(json["data"]["path"]["text"].as_str().unwrap()); // error
        features.push(Feature::UsedFeature {
            name: feature_name.to_string(),
            path: path.to_path_buf(),
            line_number,
        })
    }
    Ok(features)
}

/// A representation of a Crate.
#[derive(Debug)]
struct CrateInfo {
    // Path to the Cargo.toml file.
    path: PathBuf,
    // Set of all exposed features (represented as Strings).
    exposed_features: HashSet<Feature>,
    // Set of all used features, represented as Strings.
    used_features: HashSet<Feature>,
    // Set that represents the difference between the used features and the exposed features.
    hidden_features: HashSet<Feature>,
}

impl CrateInfo {
    /// Creates a new CrateInfo object, given a Path to its Cargo.toml file.
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

    /// Adds some features to the set of used features.
    fn add_used_features(&mut self, new_features: &HashSet<Feature>) {
        self.used_features = self.used_features.union(new_features).cloned().collect();
    }
}

fn run_rg_command(path: &Path) -> Result<Vec<u8>, String> {
    let output = Command::new("rg")
        .args(&[
            "--json",       // We want the output to be in JSON format.
            "-e",           // Specify that we wish to use a regex.
            r"feature\s*=", // The actual regex.
            "-trust",       // Specify we only wish to look for rust files.
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

/// A mapping from Paths to Crates. Only crates which USE features in their code will be added.
#[derive(Debug)]
pub struct Package(HashMap<PathBuf, CrateInfo>);

impl Package {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Finds the Cargo.toml file associated to a path.
    pub fn find_associated_cargo(&self, mut path: &Path) -> Option<PathBuf> {
        loop {
            // Create a potential candidate by appending Cargo.toml
            let candidate = path.join("Cargo.toml");
            // Check whether this file exists.
            if self.0.contains_key(&candidate) || candidate.exists() {
                // It exists, so we've found the Cargo.toml that corresponds to the initial path.
                return Some(candidate);
            } else {
                // Bad candidate: keep on looping, this time using the parent directory as path.
                path = path.parent()?;
            }
        }
    }

    /// Finds the used features by ripgrep'ing the path, looking for occurences of the pattern "feature = ".
    /// Then groups those occurences by crates.
    pub fn find_used_features(&mut self, path: &Path) -> Result<(), String> {
        // Run the command and capture its output.
        let output = run_rg_command(path)?;

        let cow = String::from_utf8_lossy(&output);
        // Output is a bunch of json separated by \n, so we split them to iterate over them.
        let jsons = cow.split('\n');

        for line in jsons {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                // Keep only the "match" jsons.
                if json
                    .get("type")
                    .expect("JSON object should have a type field")
                    == "match"
                {
                    // use get?
                    let cfg = cfg_features(&json);
                    if let Ok(v) = cfg {
                        for feature in v {
                            self.add_feature(feature)
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Adds the feature to the mapping from paths to crates.
    pub fn add_feature(&mut self, feature: Feature) {
        let path = feature.path().expect("SCOTT");
        // The path to the parent directory
        let parent = path.parent().expect("failed to find cargo file");
        // Create a Cargo.toml path candidate: a Cargo file that would be in the same directory as the .rs file we just matched.
        let cargo_path = self.find_associated_cargo(&parent).expect("SCOTT");

        if let Some(crate_info) = self.0.get_mut(&cargo_path) {
            // This crate is already in the map, so simply add the feature to the list of used features.
            crate_info.used_features.insert(feature);
        } else {
            let mut used_features_set = HashSet::new();
            // Populate the set with the list of used features.
            used_features_set.insert(feature);

            // Create a cargo entry, filled with the used vec.
            let mut crate_info = CrateInfo::new(&cargo_path);
            crate_info.add_used_features(&used_features_set);
            // Insert the Cargo entry in the path mapping.
            self.0.insert(cargo_path, crate_info);
        }
    }

    /// Finds the exposed features of every Cargo.toml file in the mapping.
    pub fn find_exposed_features(&mut self) {
        // Iterate over every Cargo.
        for v in self.0.values_mut() {
            // Load its content in a String.
            let s = read_to_string(&v.path).expect("first");
            // Parse the Cargo into a TOML structure.
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

    /// Finds the hidden features, i.e. features that are used in the code but not exposed in their corresponding Cargo.toml file.
    pub fn find_hidden_features(&mut self) {
        // Iterate over the package's crates.
        for crate_ in self.0.values_mut() {
            // Find the difference between the used features and the exposed ones, and collects it into a set.
            crate_.hidden_features = crate_
                .used_features
                .difference(&crate_.exposed_features)
                .into_iter()
                .cloned()
                .collect();
        }
    }

    // todo pretty print
    pub fn display_hidden_features(&self) {
        for cargo in self.0.values() {
            if !cargo.hidden_features.is_empty() {
                println!("path: {:?}", cargo.path);
            }
            for feature in &cargo.hidden_features {
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
        for cargo in self.0.values() {
            if !cargo.exposed_features.is_empty() {
                println!("path: {:?}", cargo.path);
            }
            for feature in &cargo.exposed_features {
                println!("\t{}", feature.name());
            }
        }
    }

    // todo pretty print
    pub fn display_used_features(&self) {
        for cargo in self.0.values() {
            if !cargo.used_features.is_empty() {
                println!("path: {:?}", cargo.path);
            }
            for feature in &cargo.used_features {
                println!(
                    "\t{}\t{}",
                    feature.name(),
                    feature.clickable_path().expect("should have path SCOTT")
                );
            }
        }
    }
}
