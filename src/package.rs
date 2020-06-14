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
            None => unreachable!(), // capture has "feature" in it, so this can't be reached.
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

// We implement Hash for Feature because we want two objects to be identical if they share the same name.
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
    /// Creates a new `CrateInfo` object, given a `Path` to its Cargo.toml file.
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
    let path_str = path
        .to_str()
        .ok_or_else(|| "Path contains non utf-8 characters")?;
    let output = Command::new("rg")
        .args(&[
            "--json",       // We want the output to be in JSON format.
            "-e",           // Specify that we wish to use a regex.
            r"feature\s*=", // The actual regex.
            "-trust",       // Specify we only wish to look for rust files.
            path_str,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        // Rg exits with a code != 0 if it doesn't find any match. This means we have 0 hidden features, so we should return Ok.
        if stderr.is_empty() {
            Ok(output.stdout)
        } else {
            Err(stderr)
        }
    }
}

/// A mapping from `PathBuf` to `CrateInfo`. Only crates which USE features in their code will be added.
#[derive(Debug)]
pub struct Package {
    mapping: HashMap<PathBuf, CrateInfo>,

    // Set of paths that are excluded.
    excluded_paths: HashSet<PathBuf>,

    // Set of features to be excluded.
    excluded_features: HashSet<String>,
}

impl Package {
    pub fn new(excluded_paths: HashSet<PathBuf>, excluded_features: HashSet<String>) -> Self {
        Self {
            mapping: HashMap::new(),
            excluded_paths,
            excluded_features,
        }
    }

    /// Finds the Cargo.toml file associated to a path.
    pub fn find_associated_cargo(&self, mut path: &Path) -> Option<PathBuf> {
        loop {
            // Create a potential candidate by appending Cargo.toml
            let candidate = path.join("Cargo.toml");
            // Check whether this file exists.
            if self.mapping.contains_key(&candidate) || candidate.exists() {
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
                    .ok_or_else(|| "JSON object should have a type field".to_string())?
                    == "match"
                {
                    // Find the features that are in the "#[cfg(...)]" declaration
                    let features = self.cfg_features(&json)?;
                    for feature in features {
                        // Make sure the feature does not appear amongst the excluded_features.
                        if !self.excluded_features.contains(feature.name()) {
                            self.add_feature(feature)?
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Adds the feature to the mapping from paths to crates.
    pub fn add_feature(&mut self, feature: Feature) -> Result<(), String> {
        let path = feature
            .path()
            .ok_or_else(|| "internal error: should have a path")?;
        // The path to the parent directory
        let parent = path.parent().ok_or_else(|| "path has no parent")?;
        // Create a Cargo.toml path candidate: a Cargo file that would be in the same directory as the .rs file we just matched.
        let cargo_path = self
            .find_associated_cargo(&parent)
            .ok_or_else(|| "could not find corresponding Cargo file")?;

        if let Some(crate_info) = self.mapping.get_mut(&cargo_path) {
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
            self.mapping.insert(cargo_path, crate_info);
        }
        Ok(())
    }

    /// Extracts the features from a JSON object.
    fn cfg_features(&self, json: &serde_json::Value) -> Result<Vec<Feature>, &'static str> {
        let path_str = json["data"]["path"]["text"]
            .as_str()
            .ok_or_else(|| "Value should be a string")?;
        let path = PathBuf::from(path_str);
        // Make sure that the path does not appear amongst the excluded paths.
        if path
            .ancestors()
            .any(|ancestor| self.excluded_paths.contains(ancestor))
        {
            return Ok(Vec::new());
        }

        // Extract the line number.
        let line_number = json["data"]["line_number"]
            .as_u64()
            .ok_or_else(|| "error converting the line number")?;

        // Extract the line in the code that contains the features (probably a #[cfg(...)]).
        let cfg_declaration = json["data"]["lines"]["text"]
            .as_str()
            .ok_or_else(|| "Value should be a string")?;

        // Get a Vector with the names of the different features declared in this line.
        let feature_names = extract_features(cfg_declaration);

        // From the vecotr of feature names, create a vector of `Feature::UsedFeature`s.
        Ok(feature_names
            .iter()
            .map(|&feature_name| Feature::UsedFeature {
                name: feature_name.to_string(),
                path: path.clone(),
                line_number,
            })
            .collect())
    }

    /// Finds the exposed features of every Cargo.toml file in the mapping.
    pub fn find_exposed_features(&mut self) {
        // Iterate over every Cargo.
        for v in self.mapping.values_mut() {
            // Load its content in a String. Using unwrap because we want our program to stop in case of an error.
            let s = read_to_string(&v.path).unwrap();
            // Parse the Cargo into a TOML structure. Using unwrap because we want our program to stop in case of an error.
            let toml = s.parse::<toml::Value>().unwrap();
            let table = match &toml.get("features") {
                Some(toml::Value::Table(table)) => Some(table),
                _ => None,
            };
            let mut exposed = HashSet::new();
            if let Some(table) = table {
                for (feature_name, _) in table.iter() {
                    let name = feature_name.to_string();
                    // Make sure the feature is not one of the excluded features.
                    if !self.excluded_features.contains(&name) {
                        exposed.insert(Feature::ExposedFeature { name });
                    };
                }
            }
            v.exposed_features = exposed;
        }
    }

    /// Finds the hidden features, i.e. features that are used in the code but not exposed in their corresponding Cargo.toml file.
    pub fn find_hidden_features(&mut self) {
        // Iterate over the package's crates.
        for crate_info in self.mapping.values_mut() {
            // Find the difference between the used features and the exposed ones, and collects it into a set.
            crate_info.hidden_features = crate_info
                .used_features
                .difference(&crate_info.exposed_features)
                .cloned()
                .collect();
        }
    }

    // todo pretty print
    pub fn check_hidden_features(&self) -> Result<(), String> {
        let mut empty = true;
        for cargo in self.mapping.values() {
            if !cargo.hidden_features.is_empty() {
                empty = false;
                println!("path: {:?}", cargo.path);
            }
            for feature in &cargo.hidden_features {
                println!(
                    "\t{}\t{}",
                    feature.name(),
                    feature
                        .clickable_path()
                        .unwrap_or_else(|| String::from(feature.name()))
                );
            }
        }
        if empty {
            Ok(())
        } else {
            Err("Hidden features detected.".to_string())
        }
    }

    // todo pretty print
    pub fn display_exposed_features(&self) {
        for cargo in self.mapping.values() {
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
        for cargo in self.mapping.values() {
            if !cargo.used_features.is_empty() {
                println!("path: {:?}", cargo.path);
            }
            for feature in &cargo.used_features {
                println!(
                    "\t{}\t{}",
                    feature.name(),
                    feature
                        .clickable_path()
                        .unwrap_or_else(|| String::from(feature.name()))
                );
            }
        }
    }
}
