//! Instructions, pseudo-instructions and their operands.

use std::fmt;

use serde::{Deserialize, Deserializer};

use crate::Error;
use crate::format::{Format, SlotKind, parse_bits};

/// An operand in `asm`: one word of the operand vocabulary.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Operand {
    Rd,
    Rs1,
    Rs2,
    Fd,
    Fs1,
    Fs2,
    Imm,
    Uimm,
    Shamt,
    Label,
    /// `imm(rs1)`
    Mem,
}

impl Operand {
    const ALL: [Operand; 11] = [
        Operand::Rd,
        Operand::Rs1,
        Operand::Rs2,
        Operand::Fd,
        Operand::Fs1,
        Operand::Fs2,
        Operand::Imm,
        Operand::Uimm,
        Operand::Shamt,
        Operand::Label,
        Operand::Mem,
    ];

    /// The names this operand is written with, each with the field it fills
    /// (`rd`, `rs1`, `rs2` or `imm`).
    pub(crate) fn words(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Operand::Rd => &[("rd", "rd")],
            Operand::Rs1 => &[("rs1", "rs1")],
            Operand::Rs2 => &[("rs2", "rs2")],
            Operand::Fd => &[("fd", "rd")],
            Operand::Fs1 => &[("fs1", "rs1")],
            Operand::Fs2 => &[("fs2", "rs2")],
            Operand::Imm => &[("imm", "imm")],
            Operand::Uimm => &[("uimm", "imm")],
            Operand::Shamt => &[("shamt", "imm")],
            Operand::Label => &[("label", "imm")],
            Operand::Mem => &[("imm", "imm"), ("rs1", "rs1")],
        }
    }

    fn parse(s: &str) -> Option<Operand> {
        Operand::ALL.into_iter().find(|o| o.to_string() == s)
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Operand::Mem => f.write_str("imm(rs1)"),
            _ => f.write_str(self.words()[0].0),
        }
    }
}

/// Deserializes `asm` such as `"rd, imm(rs1)"`.
fn operands<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Operand>, D::Error> {
    let asm = String::deserialize(d)?;
    if asm.is_empty() {
        return Ok(Vec::new());
    }
    asm.split(',')
        .map(|word| {
            let word = word.trim();
            Operand::parse(word)
                .ok_or_else(|| serde::de::Error::custom(format!("unknown operand `{word}`")))
        })
        .collect()
}

/// The values an immediate accepts: multiples of `2^align` in `[start, end)`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Range {
    pub(crate) signed: bool,
    pub(crate) width: u32,
    pub(crate) align: u32,
    pub(crate) start: i64,
    pub(crate) end: i64,
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}, {})", self.start, self.end)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawInstruction {
    name: String,
    enc: String,
    #[serde(deserialize_with = "operands")]
    asm: Vec<Operand>,
    desc: String,
    op: String,
}

pub(crate) struct Instruction {
    pub(crate) name: String,
    pub(crate) format: Format,
    /// The bits fixed by `enc`.
    pub(crate) mask: u32,
    /// The values of the bits in `mask`.
    pub(crate) pattern: u32,
    pub(crate) operands: Vec<Operand>,
    pub(crate) desc: String,
    pub(crate) op: String,
}

impl Instruction {
    pub(crate) fn new(raw: RawInstruction, formats: &[Format]) -> Result<Instruction, Error> {
        let at = format!("instruction {}", raw.name);
        let err = |msg: String| Error::Invalid {
            at: at.clone(),
            msg,
        };

        let mut tokens = raw.enc.split_whitespace();
        let name = tokens.next().unwrap_or_default();
        let format = formats
            .iter()
            .find(|f| f.name == name)
            .ok_or_else(|| err(format!("unknown format `{name}`")))?;
        let (mut mask, mut pattern) = (0, 0);
        for token in tokens {
            let (key, value) = token
                .split_once('=')
                .ok_or_else(|| err(format!("expected `field=bits`, found `{token}`")))?;
            // Instruction bits that `value` fills, from its first digit.
            let positions: Vec<u32> =
                match key.strip_prefix("imm[").and_then(|s| s.strip_suffix(']')) {
                    Some(spec) => parse_bits(spec)
                        .ok_or_else(|| err(format!("malformed `{key}`")))?
                        .into_iter()
                        .map(|bit| {
                            format
                                .imm_pos(bit)
                                .ok_or_else(|| err(format!("format {name} has no imm[{bit}]")))
                        })
                        .collect::<Result<_, _>>()?,
                    None => {
                        let slot = format
                            .field(key)
                            .ok_or_else(|| err(format!("format {name} has no field `{key}`")))?;
                        (slot.lo..=slot.hi).rev().collect()
                    }
                };
            if value.len() != positions.len() || !value.bytes().all(|c| c == b'0' || c == b'1') {
                return Err(err(format!(
                    "`{key}` takes {} binary digits, found `{value}`",
                    positions.len()
                )));
            }
            for (pos, digit) in positions.into_iter().zip(value.bytes()) {
                mask |= 1 << pos;
                pattern |= u32::from(digit == b'1') << pos;
            }
        }

        let insn = Instruction {
            name: raw.name,
            format: format.clone(),
            mask,
            pattern,
            operands: raw.asm,
            desc: raw.desc,
            op: raw.op,
        };
        for slot in &insn.format.slots {
            let field = match &slot.kind {
                SlotKind::Field(field) => field,
                SlotKind::Imm(_) => "imm",
            };
            if insn.mask & slot.mask() != slot.mask() && insn.filler(field).is_none() {
                return Err(err(format!("`{field}` is neither fixed nor an operand")));
            }
        }
        Ok(insn)
    }

    /// The operand name that fills `field` (`rd`, `rs1`, `rs2` or `imm`).
    pub(crate) fn filler(&self, field: &str) -> Option<&'static str> {
        self.operands
            .iter()
            .flat_map(|o| o.words())
            .find(|&&(_, f)| f == field)
            .map(|&(word, _)| word)
    }

    /// The value of `field` if `enc` fixes all its bits.
    pub(crate) fn field(&self, name: &str) -> Option<u32> {
        let slot = self.format.field(name)?;
        let mask = slot.mask();
        (self.mask & mask == mask).then(|| (self.pattern & mask) >> slot.lo)
    }

    /// The values the immediate operand accepts.
    pub(crate) fn range(&self) -> Option<Range> {
        let word = self.filler("imm")?;
        let free: Vec<u32> = self
            .format
            .slots
            .iter()
            .flat_map(|slot| match &slot.kind {
                SlotKind::Imm(bits) => bits
                    .iter()
                    .enumerate()
                    .filter(|&(k, _)| self.mask & (1 << (slot.hi - k as u32)) == 0)
                    .map(|(_, &bit)| bit)
                    .collect(),
                SlotKind::Field(_) => Vec::new(),
            })
            .collect();
        let (lo, hi) = (*free.iter().min()?, *free.iter().max()?);
        let signed = matches!(word, "imm" | "label");
        let (start, end) = if signed {
            (-(1 << hi), 1 << hi)
        } else {
            (0, 1 << (hi + 1))
        };
        Some(Range {
            signed,
            width: hi + 1,
            align: lo,
            start,
            end,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Pseudo {
    pub(crate) name: String,
    #[serde(rename = "asm", deserialize_with = "operands")]
    pub(crate) operands: Vec<Operand>,
    pub(crate) desc: String,
    pub(crate) op: String,
    pub(crate) expand: Vec<Candidate>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Candidate {
    #[serde(rename = "if")]
    pub(crate) cond: Option<Fits>,
    pub(crate) to: Vec<String>,
}

/// `<operand> fits <insn>`: the operand's value is in the immediate range of `insn`.
#[derive(Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct Fits {
    pub(crate) operand: Operand,
    pub(crate) insn: String,
}

impl TryFrom<String> for Fits {
    type Error = String;

    fn try_from(s: String) -> Result<Fits, String> {
        let err = || format!("expected `<operand> fits <instruction>`, found `{s}`");
        let (operand, insn) = s.split_once(" fits ").ok_or_else(err)?;
        Ok(Fits {
            operand: Operand::parse(operand).ok_or_else(err)?,
            insn: insn.to_owned(),
        })
    }
}

impl fmt::Display for Fits {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} fits {}", self.operand, self.insn)
    }
}
