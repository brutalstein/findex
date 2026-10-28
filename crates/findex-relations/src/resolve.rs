//! Resolve an imported semantic identity from a hash-verified source position.
use super::{
    currently_admitted, open, prepare, project_file, symbol_evidence_in_db, verify_path, Result,
    SymbolEvidenceReport, MAX_TYPED_RESULTS,
};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct SymbolAtReport {
    pub path: String,
    /// One-based line and zero-based UTF-8 byte column, independent of producer encoding.
    pub line: usize,
    pub column: usize,
    pub position_encoding: &'static str,
    pub resolution: &'static str,
    pub symbols: Vec<String>,
    pub evidence: Option<SymbolEvidenceReport>,
    pub stale_evidence_discarded: usize,
    pub unresolved_positions: usize,
    pub truncated: bool,
    pub interpretation: &'static str,
}

/// Translate a producer offset without accepting half a Unicode scalar/surrogate pair.
pub(super) fn utf8_column(line: &str, column: usize, encoding: i32) -> Option<usize> {
    if encoding == 1 || (encoding == 0 && line.is_ascii()) {
        return (column <= line.len() && line.is_char_boundary(column)).then_some(column);
    }
    if !matches!(encoding, 2 | 3) {
        return None;
    }
    let mut offset = 0;
    for (byte, ch) in line.char_indices() {
        if offset == column {
            return Some(byte);
        }
        offset += if encoding == 2 { ch.len_utf16() } else { 1 };
    }
    (offset == column).then_some(line.len())
}

/// Imported SCIP ranges are end-exclusive; LSP captures and legacy rows are exact
/// anchors only. Ambiguity, stale input, unknown encoding or caps never pick a winner.
pub fn symbol_at(
    project: &Path,
    relative: &Path,
    line: usize,
    column: usize,
    limit: usize,
) -> Result<SymbolAtReport> {
    if line == 0 || column > super::MAX_SOURCE_BYTES as usize {
        return Err("invalid UTF-8 source position".into());
    }
    let indexed = prepare(project)?;
    let (path, full) = project_file(project, relative)?;
    let bytes = fs::read(full)?;
    if bytes.len() as u64 > super::MAX_SOURCE_BYTES || bytes.contains(&0) {
        return Err("invalid source bytes".into());
    }
    let source = std::str::from_utf8(&bytes)?;
    let lines: Vec<_> = super::source_lines(source).collect();
    let text = lines.get(line - 1).ok_or("source line outside file")?;
    if column > text.len() || !text.is_char_boundary(column) {
        return Err("column is not a UTF-8 source boundary".into());
    }
    let mut report=SymbolAtReport {path,line,column,position_encoding:"utf8",resolution:"not_found",symbols:Vec::new(),evidence:None,
        stale_evidence_discarded:0,unresolved_positions:0,truncated:false,
        interpretation:"Source-matched imported compiler/LSP snapshot identities; compiler bindings are not recomputed by this query. No lexical fallback. A reference is not a proven call. LSP points require the exact anchor; SCIP ranges are end-exclusive."};
    let mut conn = open(project)?;
    let tx = conn.transaction()?;
    let digest: Option<String> = tx
        .query_row(
            "SELECT digest FROM evidence_files WHERE path=?1",
            params![report.path],
            |r| r.get(0),
        )
        .optional()?;
    let Some(digest) = digest else {
        let snapshot: bool =
            tx.query_row("SELECT EXISTS(SELECT 1 FROM snapshot)", [], |r| r.get(0))?;
        report.resolution = if snapshot {
            "not_imported"
        } else {
            "no_snapshot"
        };
        return Ok(report);
    };
    if blake3::hash(&bytes).to_hex().as_str() != digest
        || !currently_admitted(&indexed, &report.path, &digest)
    {
        report.resolution = "stale";
        report.stale_evidence_discarded = 1;
        return Ok(report);
    }
    let mut stmt = tx.prepare(
        "SELECT symbol,line,column,end_line,end_column,position_encoding
        FROM semantic_sites WHERE path=?1 AND line<=?2 AND COALESCE(end_line,line)>=?2
        ORDER BY line DESC,column,symbol LIMIT ?3",
    )?;
    let entries = stmt
        .query_map(
            params![report.path, line as i64, (MAX_TYPED_RESULTS + 1) as i64],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, usize>(1)?,
                    r.get::<_, usize>(2)?,
                    r.get::<_, Option<usize>>(3)?,
                    r.get::<_, Option<usize>>(4)?,
                    r.get::<_, i32>(5)?,
                ))
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    report.truncated = entries.len() > MAX_TYPED_RESULTS;
    let max = limit.clamp(1, MAX_TYPED_RESULTS);
    let mut matching = Vec::new();
    for (symbol, start_line, start, end_line, end, encoding) in
        entries.into_iter().take(MAX_TYPED_RESULTS)
    {
        let start = lines
            .get(start_line.saturating_sub(1))
            .and_then(|s| utf8_column(s, start, encoding));
        let has_end = end.is_some();
        let end_line = end_line.unwrap_or(start_line);
        let end = end.and_then(|c| {
            lines
                .get(end_line.saturating_sub(1))
                .and_then(|s| utf8_column(s, c, encoding))
        });
        let Some(start) = start else {
            report.unresolved_positions += 1;
            continue;
        };
        if has_end && end.is_none() {
            report.unresolved_positions += 1;
            continue;
        }
        let contains = match end {
            Some(end) if (start_line, start) != (end_line, end) => {
                (start_line, start) <= (line, column) && (line, column) < (end_line, end)
            }
            Some(_) => (start_line, start) == (line, column),
            None => (start_line, start) == (line, column),
        };
        if contains {
            matching.push((
                symbol,
                (start_line, start),
                (end_line, end.unwrap_or(start)),
            ));
        }
    }
    let mut seen = BTreeSet::new();
    // A compiler may attach a module-wide range around a token range. Only
    // strict containment loses precedence; equal or crossing ranges stay ambiguous.
    for (symbol, start, end) in &matching {
        if matching.iter().any(|(_, inner_start, inner_end)| {
            start <= inner_start && inner_end <= end && (start, end) != (inner_start, inner_end)
        }) {
            continue;
        }
        if seen.insert(symbol.clone()) {
            if report.symbols.len() < max {
                report.symbols.push(symbol.clone());
            } else {
                report.truncated = true;
            }
        }
    }
    report.resolution = if report.truncated || report.unresolved_positions > 0 {
        "incomplete"
    } else if report.symbols.len() > 1 {
        "ambiguous"
    } else if report.symbols.is_empty() {
        "not_found"
    } else {
        "resolved"
    };
    if report.resolution == "resolved" {
        let evidence = symbol_evidence_in_db(project, &report.symbols[0], max, &indexed, &tx)?;
        report.truncated |= evidence.truncated;
        report.stale_evidence_discarded += evidence.stale_evidence_discarded;
        report.evidence = Some(evidence);
    }
    if !verify_path(project, &report.path, &digest) {
        report.resolution = "stale";
        report.stale_evidence_discarded += 1;
        report.symbols.clear();
        report.evidence = None;
    }
    Ok(report)
}
