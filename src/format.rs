//! Instruction formats: how the fields lie in a 32-bit instruction word.

use indexmap::IndexMap;

use crate::Error;

pub(crate) struct Format {
    pub(crate) name: String,
    /// From the most significant bit.
    pub(crate) slots: Vec<Slot>,
}

/// A field of a format, at instruction bits `hi` down to `lo`.
pub(crate) struct Slot {
    pub(crate) hi: u32,
    pub(crate) lo: u32,
    pub(crate) kind: SlotKind,
}

pub(crate) enum SlotKind {
    Field(String),
    /// `bits[k]` is the immediate bit placed at instruction bit `hi - k`.
    Imm(Vec<u32>),
}

impl Format {
    /// Parses a layout such as `"imm[11:5] rs2 rs1 funct3 imm[4:0] opcode"`.
    pub(crate) fn parse(
        name: &str,
        layout: &str,
        fields: &IndexMap<String, u32>,
    ) -> Result<Format, Error> {
        let err = |msg: String| Error::Invalid {
            at: format!("format {name}"),
            msg,
        };
        let mut pos: u32 = 32;
        let mut slots = Vec::new();
        for token in layout.split_whitespace() {
            let (kind, width) = match token.strip_prefix("imm[").and_then(|s| s.strip_suffix(']')) {
                Some(spec) => {
                    let bits =
                        parse_bits(spec).ok_or_else(|| err(format!("malformed `{token}`")))?;
                    let width = bits.len() as u32;
                    (SlotKind::Imm(bits), width)
                }
                None => {
                    let &width = fields
                        .get(token)
                        .ok_or_else(|| err(format!("unknown field `{token}`")))?;
                    (SlotKind::Field(token.to_owned()), width)
                }
            };
            let lo = pos
                .checked_sub(width)
                .ok_or_else(|| err("wider than 32 bits".to_owned()))?;
            slots.push(Slot {
                hi: pos - 1,
                lo,
                kind,
            });
            pos = lo;
        }
        if pos != 0 {
            return Err(err(format!("{} bits wide, not 32", 32 - pos)));
        }
        Ok(Format {
            name: name.to_owned(),
            slots,
        })
    }

    /// The instruction bit that holds immediate bit `bit`.
    pub(crate) fn imm_pos(&self, bit: u32) -> Option<u32> {
        self.slots.iter().find_map(|slot| match &slot.kind {
            SlotKind::Imm(bits) => bits
                .iter()
                .position(|&b| b == bit)
                .map(|k| slot.hi - k as u32),
            SlotKind::Field(_) => None,
        })
    }
}

/// `"12|10:5"` → `[12, 10, 9, 8, 7, 6, 5]`.
pub(crate) fn parse_bits(spec: &str) -> Option<Vec<u32>> {
    let mut bits = Vec::new();
    for part in spec.split('|') {
        match part.split_once(':') {
            Some((hi, lo)) => {
                let (hi, lo): (u32, u32) = (hi.parse().ok()?, lo.parse().ok()?);
                if hi < lo {
                    return None;
                }
                bits.extend((lo..=hi).rev());
            }
            None => bits.push(part.parse().ok()?),
        }
    }
    Some(bits)
}

/// The inverse of [`parse_bits`].
pub(crate) fn format_bits(bits: &[u32]) -> String {
    bits.chunk_by(|&a, &b| a == b + 1)
        .map(|run| match run {
            [hi, .., lo] => format!("{hi}:{lo}"),
            _ => run[0].to_string(),
        })
        .collect::<Vec<_>>()
        .join("|")
}
