use std::path::PathBuf;

use crate::cli::paths::{default_install_location, default_output_location};
use crate::generate::{generate_json, generate_kdl, serialize_kdl};
use crate::schema::{JsonThemeFamily, KdlThemeFamily};
use crate::util::LogExpect;
use anyhow::{anyhow, Result as Res};
use clap::{Parser, ValueEnum};
use log::{debug, error, info, warn};
use notify::event::{AccessKind, AccessMode, Event};
use notify::{EventKind, Watcher};
use std::fs::{File, OpenOptions};
use std::io::BufWriter;
use std::path::Path;

/// A helper tool for making Zed themes using a custom KDL
/// format that allows naming colors, reusing components, and much
/// more
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    /// The input file used to generate a new theme file
    infile: PathBuf,
    /// The output file for the generated file. This is not the final install location.
    /// Creates parent directories if they do not exist.
    /// Defaults to `./generated/{relative-path-to-file}.{extension}`.
    #[arg(short, long)]
    outfile: Option<PathBuf>,
    /// The install location for the theme after generation. By default, it
    /// is automatically detected the same way that Zed does it.
    #[arg(short, long)]
    install_location: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Parser, Debug, PartialEq)]
pub enum Command {
    /// Generates a theme family JSON file from a KDL `infile`
    Generate,
    /// Generates a theme family from a KDL `infile` and installs it. Note that this does not
    /// generate an extension from the theme: it just simply generates the JSON file.
    Install,
    /// Watches for changes on the KDL `infile`, generates a theme from it,
    /// and installs it into `install_location`, allowing
    /// for a hot swap loop if the theme is selected.
    Watch,
    /// Converts an existing JSON theme family into the custom KDL format. It attempts
    /// to extract all colors into a palette and names the colors at best effort.
    Migrate,
    /// Writes the palette of a theme file to standard output in a given format
    ExportPalette {
        /// The format to export to
        #[arg(value_enum)]
        format: PaletteFormat,
    },
}

#[derive(ValueEnum, Debug, PartialEq, Clone)]
pub enum PaletteFormat {
    /// Export as a Rust-style array of tuples
    ArrayOfTuples,
    /// Export as a newline-separated list of `name color`
    SpaceSeparated,
}

impl PaletteFormat {
    fn output(&self, infile: &Path) -> Res<()> {
        debug!("Reading KDL data from {}", infile.display());
        let kdl = KdlThemeFamily::read(infile)?;
        let palette = kdl.palette.into_palette().resolve()?;

        let mut data = palette
            .colors
            .into_iter()
            .map(|(name, color)| (name, color.to_string()))
            .collect::<Vec<_>>();
        data.sort_unstable_by(|(key1, _), (key2, _)| key1.cmp(key2));

        match self {
            Self::ArrayOfTuples => {
                print!("{data:?}");
                Ok(())
            }
            Self::SpaceSeparated => {
                for (name, color) in data {
                    println!("{name} {color}");
                }
                Ok(())
            }
        }
    }
}
fn generate_json_cmd(infile: &Path, outfile: &Path) -> Res<()> {
    debug!("Reading KDL data from {}", infile.display());
    let kdl = KdlThemeFamily::read(infile)?;
    let json = generate_json(kdl)?;
    debug!("Writing JSON data to {}", outfile.display());
    let prefix = outfile
        .parent()
        .ok_or_else(|| anyhow!("output file has no parent"))?;
    std::fs::create_dir_all(prefix)?;

    let writer = BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(outfile)?,
    );
    serde_json::to_writer_pretty(writer, &json)?;
    Ok(())
}

fn install_cmd(infile: &Path, outfile: &Path, installfile: &Path) -> Res<()> {
    generate_json_cmd(infile, outfile)?;
    std::fs::copy(outfile, installfile)?;
    Ok(())
}

fn watch_cmd(infile: &Path, outfile: &Path, installfile: &Path) -> Res<()> {
    info!("Watching for changes on {}", infile.display());

    let (tx, rx) = std::sync::mpsc::channel::<notify::Result<Event>>();
    let mut watcher = notify::recommended_watcher(tx)?;
    watcher.watch(infile, notify::RecursiveMode::NonRecursive)?;

    for res in rx {
        let res = match res {
            Ok(res) => res.kind,
            Err(e) => {
                warn!("Error while watching file: {e}. Continuing to wait...");
                continue;
            }
        };
        match res {
            // we only want to update after closing with write permissions. If we listened for other modification events,
            // we would not only receive surplus events, but not have the full contents of the file.
            EventKind::Access(AccessKind::Close(AccessMode::Write)) => {
                debug!("{} was modified. Updating...", infile.display());

                match install_cmd(infile, outfile, installfile) {
                    Ok(()) => {}
                    Err(e) => {
                        warn!("Failed to update: {e}");
                    }
                }
            }
            EventKind::Access(_) => {
                debug!("{} was accessed. Ignoring...", infile.display());
            }
            EventKind::Create(_) => {
                debug!("{} was created. Ignoring...", infile.display());
            }
            EventKind::Remove(_) => {
                error!("{} was deleted. Stopping...", infile.display());
                return Ok(());
            }
            _ => {}
        }
    }
    Ok(())
}

fn migrate_cmd(infile: &Path, outfile: &Path) -> Res<()> {
    let reader = File::open(infile)?;
    let json: JsonThemeFamily = serde_json::from_reader(reader)?;
    let file = generate_kdl(json);

    let prefix = outfile
        .parent()
        .ok_or_else(|| anyhow!("Output file has no parent"))?;
    std::fs::create_dir_all(prefix)?;
    let writer = BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(outfile)?,
    );
    serialize_kdl(writer, &file)?;
    Ok(())
}

impl Cli {
    pub fn run(self) {
        let Cli {
            command,
            infile,
            install_location,
            outfile,
        } = self;
        let ext = if command == Command::Migrate {
            "kdl"
        } else {
            "json"
        };
        let outfile = outfile.unwrap_or_else(|| {
            debug!(
                "User did not provide an outfile, generating default based on the input file {}",
                infile.display()
            );
            default_output_location(&infile, ext)
                .log_expect("Error generating output file location")
        });
        let install_location = install_location.unwrap_or_else(|| {
            debug!(
                "User did not provide an install location, defaulting to the default Zed config path based on the output file `{}`",
                infile.display()
            );
            default_install_location(&outfile).log_expect("Error generating install location")
        });

        match command {
            Command::Generate => {
                generate_json_cmd(&infile, &outfile).log_expect("Could not write JSON file");
            }
            Command::Install => {
                install_cmd(&infile, &outfile, &install_location)
                    .log_expect("Failed to install theme");
            }
            Command::Watch => {
                watch_cmd(&infile, &outfile, &install_location).log_expect("Failed to watch file");
            }
            Command::Migrate => {
                migrate_cmd(&infile, &outfile).log_expect("Failed to migrate theme");
            }
            Command::ExportPalette { format } => {
                format.output(&infile).log_expect("Failed to write data");
            }
        }
    }
}
