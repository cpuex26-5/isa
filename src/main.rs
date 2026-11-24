use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use isa::Isa;

/// Tools for the cpuex26-5 ISA.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Read this isa.toml instead of the embedded one
    #[arg(long, global = true)]
    isa: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate the ISA doc as <OUT>/index.html
    Doc {
        #[arg(short, long, default_value = "site")]
        out: PathBuf,
        /// The commit the doc is generated from, linked from its header
        #[arg(long)]
        commit: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let isa = match &cli.isa {
        Some(path) => {
            let src = fs::read_to_string(path)
                .with_context(|| format!("cannot read {}", path.display()))?;
            Isa::parse(&src).with_context(|| format!("invalid {}", path.display()))?
        }
        None => Isa::embedded().context("invalid embedded isa.toml")?,
    };
    match cli.command {
        Command::Doc { out, commit } => {
            let html = isa::doc::render(&isa, jiff::ZonedDateTime::now().date(), commit.as_deref());
            fs::create_dir_all(&out).with_context(|| format!("cannot create {}", out.display()))?;
            let path = out.join("index.html");
            fs::write(&path, html).with_context(|| format!("cannot write {}", path.display()))?;
        }
    }
    Ok(())
}
