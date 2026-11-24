//! The ISA doc: a single HTML page generated from `isa.toml`.

use std::collections::BTreeSet;

use maud::{DOCTYPE, Markup, PreEscaped, html};

use crate::Isa;
use crate::format::{Format, SlotKind, format_bits};
use crate::insn::{Candidate, Instruction, Operand};

const CSS: &str = include_str!("doc.css");
const FONTS: &str = "https://fonts.googleapis.com/css2?family=IBM+Plex+Sans+JP:wght@400;500;600;700&family=JetBrains+Mono:ital,wght@0,400;0,600;0,700;1,400&display=swap";
const REPO: &str = env!("CARGO_PKG_REPOSITORY");

/// Renders the doc of `isa`, generated on `date` from `commit`.
pub fn render(isa: &Isa, date: jiff::civil::Date, commit: Option<&str>) -> String {
    let cols = columns(isa);
    html! {
        (DOCTYPE)
        html lang="ja" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "cpuex26-5/isa" }
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link rel="stylesheet" href=(FONTS);
                style { (PreEscaped(CSS)) }
            }
            body {
                div.pg {
                    (header(date, commit))
                    section id="pg-fmt" class="pg-intro" {
                        div { h2 { "命令形式" } (formats(isa, &cols)) }
                        div { h2 { "記法" } (notation(isa)) }
                    }
                    section #pg-ins { h2 { "命令一覧" } (instructions(isa, &cols)) }
                    section #pg-ps { h2 { "擬似命令" } (pseudos(isa)) }
                }
            }
        }
    }
    .into_string()
}

fn header(date: jiff::civil::Date, commit: Option<&str>) -> Markup {
    let nav = [
        ("pg-fmt", "命令形式"),
        ("pg-ins", "命令一覧"),
        ("pg-ps", "擬似命令"),
        ("pg-abi", "ABI"),
    ];
    html! {
        header.pg-top {
            div { span.pg-title { "cpuex26-5/isa" } }
            nav { @for (id, title) in nav { a href={ "#" (id) } { (title) } } }
            div.pg-meta {
                (date) " ・ "
                @if let Some(commit) = commit {
                    "isa.toml @ "
                    a.sha href={ (REPO) "/blob/" (commit) "/isa.toml" } { (commit.get(..7).unwrap_or(commit)) }
                    " ・ "
                }
                a href={ (REPO) "/releases" } { "Releases" }
            }
        }
    }
}

/// A run of instruction bits `hi` down to `lo` shown as one table cell.
struct Cell<'a> {
    class: &'a str,
    text: String,
    hi: u32,
    lo: u32,
}

fn format_cells(format: &Format) -> Vec<Cell<'_>> {
    format
        .slots
        .iter()
        .map(|slot| {
            let (class, text) = match &slot.kind {
                SlotKind::Imm(bits) => ("imm", format!("imm[{}]", format_bits(bits))),
                SlotKind::Field(field) if matches!(field.as_str(), "rd" | "rs1" | "rs2") => {
                    (field.as_str(), field.clone())
                }
                SlotKind::Field(field) => ("fix", field.clone()),
            };
            Cell {
                class,
                text,
                hi: slot.hi,
                lo: slot.lo,
            }
        })
        .collect()
}

/// Fixed bits show their values; the others show the operand that fills them.
fn insn_cells(insn: &Instruction) -> Vec<Cell<'_>> {
    let fixed = |pos: u32| insn.mask >> pos & 1 == 1;
    let fix = |hi: u32, lo: u32| Cell {
        class: "fix",
        text: (lo..=hi)
            .rev()
            .map(|pos| {
                if insn.pattern >> pos & 1 == 1 {
                    '1'
                } else {
                    '0'
                }
            })
            .collect(),
        hi,
        lo,
    };
    let filler = |field: &str| insn.filler(field).expect("checked by Instruction::new");
    let mut cells = Vec::new();
    for slot in &insn.format.slots {
        match &slot.kind {
            SlotKind::Field(field) if insn.field(field).is_some() => {
                cells.push(fix(slot.hi, slot.lo))
            }
            SlotKind::Field(field) => cells.push(Cell {
                class: field,
                text: filler(field).to_owned(),
                hi: slot.hi,
                lo: slot.lo,
            }),
            SlotKind::Imm(bits) => {
                let placed: Vec<(u32, u32)> = bits
                    .iter()
                    .enumerate()
                    .map(|(k, &bit)| (slot.hi - k as u32, bit))
                    .collect();
                for run in placed.chunk_by(|a, b| fixed(a.0) == fixed(b.0)) {
                    let (hi, lo) = (run[0].0, run[run.len() - 1].0);
                    if fixed(hi) {
                        cells.push(fix(hi, lo));
                    } else {
                        let imm: Vec<u32> = run.iter().map(|&(_, bit)| bit).collect();
                        let text = format!("{}[{}]", filler("imm"), format_bits(&imm));
                        cells.push(Cell {
                            class: "imm",
                            text,
                            hi,
                            lo,
                        });
                    }
                }
            }
        }
    }
    cells
}

/// The bit columns `(hi, lo)`: every cell boundary of every format and instruction.
fn columns(isa: &Isa) -> Vec<(u32, u32)> {
    let cells = isa
        .formats
        .iter()
        .flat_map(format_cells)
        .chain(isa.instructions.iter().flat_map(insn_cells));
    let edges: BTreeSet<u32> = cells
        .flat_map(|cell| [cell.hi + 1, cell.lo])
        .chain([32])
        .collect();
    let edges: Vec<u32> = edges.into_iter().rev().collect();
    edges.windows(2).map(|w| (w[0] - 1, w[1])).collect()
}

fn column_heads(cols: &[(u32, u32)]) -> Markup {
    html! {
        @for &(hi, lo) in cols {
            th { @if hi == lo { (hi) } @else { (hi) " · " (lo) } }
        }
    }
}

fn cells(cells: &[Cell], cols: &[(u32, u32)]) -> Markup {
    html! {
        @for cell in cells {
            @let span = cols.iter().filter(|&&(hi, lo)| hi <= cell.hi && lo >= cell.lo).count();
            td class={ "b " (cell.class) } colspan=[(span > 1).then_some(span)] { (cell.text) }
        }
    }
}

fn formats(isa: &Isa, cols: &[(u32, u32)]) -> Markup {
    html! {
        table.pg-t.pg-fmt {
            thead { tr { th {} (column_heads(cols)) } }
            tbody {
                @for format in &isa.formats {
                    tr { td.fname { (format.name) } (cells(&format_cells(format), cols)) }
                }
            }
        }
    }
}

fn notation(isa: &Isa) -> Markup {
    html! {
        dl.pg-gloss {
            @for word in ["imm", "uimm", "shamt", "label"] {
                @let lines = ranges(isa, word);
                @if !lines.is_empty() {
                    dt { span.t-imm { (word) } }
                    dd { @for (i, line) in lines.iter().enumerate() { @if i > 0 { br; } (line) } }
                }
            }
            @for entry in &isa.notation {
                dt { (entry.sym) }
                dd { (entry.desc) }
            }
        }
    }
}

/// One line per distinct range of the immediates written `word`, naming the formats that use it.
fn ranges(isa: &Isa, word: &str) -> Vec<String> {
    let mut groups: Vec<(_, Vec<&str>)> = Vec::new();
    for insn in isa
        .instructions
        .iter()
        .filter(|insn| insn.filler("imm") == Some(word))
    {
        let Some(range) = insn.range() else { continue };
        let format = insn.format.name.as_str();
        match groups.iter_mut().find(|(r, _)| *r == range) {
            Some((_, formats)) if formats.contains(&format) => {}
            Some((_, formats)) => formats.push(format),
            None => groups.push((range, vec![format])),
        }
    }
    let what = if word == "label" {
        "pc-relative offset"
    } else {
        "immediate"
    };
    groups
        .iter()
        .map(|(range, formats)| {
            let align = match range.align {
                0 => String::new(),
                1 => "even, ".to_owned(),
                n => format!("multiple of {}, ", 1 << n),
            };
            let sign = if range.signed { "signed" } else { "unsigned" };
            format!(
                "{}-type {what}, {align}{sign} {} bits, {range}",
                formats.join("/"),
                range.width
            )
        })
        .collect()
}

fn instructions(isa: &Isa, cols: &[(u32, u32)]) -> Markup {
    html! {
        table.pg-t.pg-ins {
            thead {
                tr {
                    th.fm { "Format" }
                    (column_heads(cols))
                    th.l { "Instruction" }
                    th.l { "Description" }
                    th.l { "Operation" }
                }
            }
            tbody {
                @for (i, insn) in isa.instructions.iter().enumerate() {
                    // A line above each row whose opcode differs from the previous row.
                    @let sep = i > 0 && insn.field("opcode") != isa.instructions[i - 1].field("opcode");
                    tr id={ "i-" (insn.name) } class=[sep.then_some("sep")] {
                        td.fm { (insn.format.name) }
                        (cells(&insn_cells(insn), cols))
                        td.syn { a href={ "#i-" (insn.name) } { (syntax(&insn.name, &insn.operands)) } }
                        td.desc { (insn.desc) }
                        td.sem { (paint(&insn.op, &insn.operands)) }
                    }
                }
            }
        }
    }
}

fn pseudos(isa: &Isa) -> Markup {
    html! {
        table.pg-t.pg-ps {
            thead {
                tr {
                    th.l { "Pseudo-instruction" }
                    th.l { "Description" }
                    th.l { "Operation" }
                    th.l { "Expansion" }
                }
            }
            tbody {
                @for ps in &isa.pseudo {
                    tr id={ "p-" (ps.name) } {
                        td.syn { a href={ "#p-" (ps.name) } { (syntax(&ps.name, &ps.operands)) } }
                        td.desc { (ps.desc) }
                        td.sem { (paint(&ps.op, &ps.operands)) }
                        td.exp {
                            @for cand in &ps.expand {
                                div.blk {
                                    @for (k, line) in cand.to.iter().enumerate() {
                                        @let (mnemonic, rest) = line.split_once(' ').unwrap_or((line, ""));
                                        div.ln {
                                            span.xm { (mnemonic) }
                                            (paint(rest, &ps.operands))
                                            @if ps.expand.len() > 1 && k == 0 {
                                                span.cmt { (condition(isa, cand)) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The comment on the first line of an expansion candidate.
fn condition(isa: &Isa, cand: &Candidate) -> String {
    let Some(fits) = &cand.cond else {
        return "# otherwise".to_owned();
    };
    match isa.instruction(&fits.insn).and_then(Instruction::range) {
        Some(range) => format!("# if {fits} {range}"),
        None => format!("# if {fits}"),
    }
}

/// `add rd, rs1, rs2` with the mnemonic in bold and the operands coloured.
fn syntax(name: &str, operands: &[Operand]) -> Markup {
    let asm = operands
        .iter()
        .map(Operand::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    html! {
        span.mn { (name) }
        @if !asm.is_empty() { " " (paint(&asm, operands)) }
    }
}

/// Colours the words of `text` that name one of `operands` by the field they fill.
fn paint(text: &str, operands: &[Operand]) -> Markup {
    let field = |token: &str| {
        operands
            .iter()
            .flat_map(|o| o.words())
            .find(|&&(word, _)| word == token)
            .map(|&(_, field)| field)
    };
    html! {
        @for token in tokens(text) {
            @match field(token) {
                Some(field) => span class={ "t-" (field) } { (token) },
                None => (token),
            }
        }
    }
}

/// Splits `text` into words (alphanumerics and `_`) and the runs between them.
fn tokens(text: &str) -> Vec<&str> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut tokens = Vec::new();
    let mut start = 0;
    for (i, c) in text.char_indices().skip(1) {
        let prev = text[..i].chars().next_back().is_some_and(is_word);
        if is_word(c) != prev {
            tokens.push(&text[start..i]);
            start = i;
        }
    }
    if start < text.len() {
        tokens.push(&text[start..]);
    }
    tokens
}
