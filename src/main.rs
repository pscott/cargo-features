use std::path::PathBuf;
mod package;
use package::Package;
use structopt::StructOpt;
mod tests;

/// Helper function to parse a boolean from a &str. Used when parsing the args from the command line.
fn true_or_false(s: &str) -> Result<bool, &'static str> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("expected `true` or `false`"),
    }
}

/// Struct that contains a number of options used during execution.
#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(parse(from_os_str), default_value = ".")]
    path: PathBuf,

    #[structopt(short, long, parse(try_from_str = true_or_false), default_value = "true")]
    hidden_features: bool,

    #[structopt(long)]
    exposed_features: bool,

    #[structopt(short, long)]
    used_features: bool,

    #[structopt(long)]
    excluded_paths: Vec<String>,

    #[structopt(long)]
    excluded_features: Vec<String>,
}

fn main() -> Result<(), String> {
    let opt = Opt::from_args();

    let excluded_paths = opt.excluded_paths.iter().cloned().collect();
    let excluded_features = opt.excluded_features.iter().cloned().collect();
    let mut package = Package::new(excluded_paths, excluded_features);
    package.find_used_features(&opt.path)?;
    package.find_exposed_features();
    package.find_hidden_features();
    if opt.exposed_features {
        package.display_exposed_features();
    }
    if opt.used_features {
        package.display_used_features();
    }
    package.check_hidden_features()
}
