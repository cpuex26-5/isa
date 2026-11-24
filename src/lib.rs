//! The cpuex26-5 ISA, loaded from `isa.toml`.
#![expect(dead_code, reason = "read by the doc generator")]

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer};

/// The ISA described by `isa.toml`.
pub struct Isa {
    pub(crate) notation: Vec<Notation>,
    pub(crate) formats: IndexMap<String, String>,
    pub(crate) registers: Registers,
    pub(crate) mmio: Vec<Mmio>,
    pub(crate) instructions: Vec<RawInstruction>,
    pub(crate) pseudo: Vec<toml::Value>,
    pub(crate) notes: Vec<Note>,
}

impl Isa {
    /// Parses the text of an `isa.toml`.
    pub fn parse(src: &str) -> Result<Isa, Error> {
        let src: Source = toml::from_str(src)?;
        Ok(Isa {
            notation: src.notation,
            formats: src.formats,
            registers: src.registers,
            mmio: src.mmio,
            instructions: src.instructions,
            pseudo: src.pseudo,
            notes: src.notes,
        })
    }

    /// The `isa.toml` of this repository, embedded at build time.
    pub fn embedded() -> Result<Isa, Error> {
        Isa::parse(include_str!("../isa.toml"))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
    #[error("{at}: {msg}")]
    Invalid { at: String, msg: String },
}

/// `isa.toml` as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    notation: Vec<Notation>,
    fields: IndexMap<String, u32>,
    formats: IndexMap<String, String>,
    registers: Registers,
    mmio: Vec<Mmio>,
    instructions: Vec<RawInstruction>,
    pseudo: Vec<toml::Value>,
    notes: Vec<Note>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Notation {
    pub(crate) sym: String,
    pub(crate) desc: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Registers {
    pub(crate) x: Vec<Register>,
    pub(crate) f: Vec<Register>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Register {
    #[serde(deserialize_with = "one_or_many")]
    pub(crate) abi: Vec<String>,
    pub(crate) role: String,
    pub(crate) saver: Option<Saver>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Saver {
    Caller,
    Callee,
}

impl Saver {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Saver::Caller => "caller",
            Saver::Callee => "callee",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Mmio {
    pub(crate) addr: u32,
    pub(crate) name: String,
    pub(crate) dir: Access,
    pub(crate) desc: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Access {
    R,
    W,
    Rw,
}

impl Access {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Access::R => "r",
            Access::W => "w",
            Access::Rw => "rw",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Note {
    pub(crate) title: String,
    pub(crate) body: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawInstruction {
    pub(crate) name: String,
    pub(crate) enc: String,
    pub(crate) asm: String,
    pub(crate) desc: String,
    pub(crate) op: String,
}

fn one_or_many<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    Ok(match OneOrMany::deserialize(d)? {
        OneOrMany::One(s) => vec![s],
        OneOrMany::Many(v) => v,
    })
}
