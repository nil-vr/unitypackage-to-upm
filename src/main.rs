mod manifest;
mod unity_package;
mod upm;

use clap::Parser;
use manifest::Manifest;
use miette::{bail, Context, IntoDiagnostic, Result};
use std::{
    fs::File,
    io::{prelude::*, BufReader},
    path::PathBuf,
};
use tracing::info_span;
use unity_package::PackageEntry;
use upm::PackageBuilder;

#[derive(Parser)]
#[clap(author, version, about)]
struct CliArgs {
    /// The path to the .unitypackage file.
    #[clap(parse(from_os_str), value_name = "UNITY_PACKAGE")]
    package: PathBuf,
    /// The path to the package.json describing the UPM package.
    #[clap(parse(from_os_str), value_name = "PACKAGE_JSON")]
    vpm_json: PathBuf,
    /// The path to write the converted package.
    #[clap(parse(from_os_str), value_name = "UPM_PACKAGE")]
    vpm: PathBuf,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = CliArgs::parse();

    let _ = info_span!("Converting package {package}", package = ?args.package).enter();

    let package = File::open(&args.package)
        .into_diagnostic()
        .wrap_err("Failed to open Unity package")?;
    let mut package = unity_package::Package::new(package);
    let package_entries = package.entries().wrap_err("Failed to read Unity package")?;

    let mut manifest_string = String::new();
    {
        let manifest = File::open(&args.vpm_json)
            .into_diagnostic()
            .wrap_err("Failed to open package.json")?;
        let mut manifest = BufReader::new(manifest);
        manifest
            .read_to_string(&mut manifest_string)
            .into_diagnostic()
            .wrap_err("Failed to read manifest")?;
    }

    let manifest = Manifest::parse(&manifest_string, &args.vpm_json.to_string_lossy())
        .wrap_err("Failed to parse manifest")?;

    let mut vpm = {
        let vpm = File::create(&args.vpm)
            .into_diagnostic()
            .wrap_err("Failed to create VPM package")?;
        PackageBuilder::new(vpm, format!("{}@{}", manifest.name, manifest.version))
    };

    vpm.append("package.json", &mut manifest_string.as_bytes())
        .wrap_err("Failed to write package.json")?;

    let mut failed = false;
    for entry in package_entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                eprintln!("{}", error);
                failed = true;
                continue;
            }
        };

        let PackageEntry { path, mut content } = entry;
        vpm.append(&path, &mut content)
            .wrap_err_with(|| format!("Failed to process {path:?}"))?;
    }

    vpm.finish().wrap_err("Failed to close VPM file")?;

    if failed {
        bail!("One or more entries could not be proccessed.");
    }

    Ok(())
}
