mod package;
mod test;

use package::Package;
use std::collections::HashSet;
use std::path::PathBuf;
use structopt::StructOpt;

/// Struct that contains a number of options used during execution.
#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(parse(from_os_str), default_value = ".")]
    path: PathBuf,

    #[structopt(long)]
    ignored_paths: Vec<String>,

    #[structopt(long)]
    ignored_features: Vec<String>,
}

fn main() -> Result<(), String> {
    let opt = Opt::from_args();

    let mut ignored_paths: HashSet<PathBuf> = opt
        .ignored_paths
        .iter()
        .cloned()
        .map(PathBuf::from)
        .collect();

    // Ignore the "target" directoy.
    ignored_paths.insert(opt.path.join("target"));

    let ignored_features = opt.ignored_features.iter().cloned().collect();

    let mut package = Package::new(ignored_paths, ignored_features);
    package.find_used_features(&opt.path)?;
    package.find_exposed_features();
    package.find_hidden_features();
    package.check_hidden_features()
}
