use std::path::PathBuf;
mod package;
use package::Package;
use structopt::StructOpt;
mod tests;

fn true_or_false(s: &str) -> Result<bool, &'static str> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("expected `true` or `false`"),
    }
}

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(parse(from_os_str), default_value = ".")]
    path: PathBuf,

    #[structopt(short, long, parse(try_from_str = true_or_false), default_value = "true")]
    hidden_features: bool,

    #[structopt(short, long)]
    exposed_features: bool,

    #[structopt(short, long)]
    used_features: bool,
}

struct Foo {
    a: Box<Vec<String>>,
}

fn main() -> Result<(), String> {
    let opt = Opt::from_args();

    let mut v = Vec::new();
    v.push(String::from("a"));

    let f = Foo { a: Box::new(v) };
    println!("{:?}", f.a);
    let mut package = Package::new();
    package.find_used_features(&opt.path)?;
    package.find_exposed_features();
    package.find_hidden_features();
    if opt.hidden_features {
        package.display_hidden_features();
    }
    if opt.exposed_features {
        package.display_exposed_features();
    }
    if opt.used_features {
        package.display_used_features();
    }
    Ok(())
}
