#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Source identity, byte span, and source-location utilities shared by the
//! compiler crates.
//!
//! The compiler keeps spans as byte offsets into a `SourceFile`, while public
//! diagnostics and code frames can project those offsets to one-based line and
//! UTF-16 column locations compatible with Vue and JavaScript tooling.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Stable identifier for a file registered in a `SourceMap`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FileId(pub u32);

/// UTF-8 byte offset into a source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BytePos(pub usize);

/// One-based line and UTF-16 column location.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loc {
    /// One-based line number.
    pub line: usize,
    /// One-based UTF-16 column number.
    pub column: usize,
}

/// A pair of one-based source locations for a byte span.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocSpan {
    /// Location of the inclusive span start.
    pub start: Loc,
    /// Location of the exclusive span end.
    pub end: Loc,
}

/// Half-open byte span inside a source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// File that owns this span.
    pub file_id: FileId,
    /// Inclusive start byte offset.
    pub start: BytePos,
    /// Exclusive end byte offset.
    pub end: BytePos,
}

impl Span {
    /// Creates a span from raw byte offsets in the given file.
    pub const fn new(file_id: FileId, start: usize, end: usize) -> Self {
        Self {
            file_id,
            start: BytePos(start),
            end: BytePos(end),
        }
    }

    /// Returns the span length in bytes.
    pub fn len(self) -> usize {
        self.end.0.saturating_sub(self.start.0)
    }

    /// Returns `true` when the span is empty.
    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// Returns a span shifted by `delta` bytes in the same file.
    pub fn translate(self, delta: isize) -> Option<Self> {
        let start = translate_offset(self.start.0, delta)?;
        let end = translate_offset(self.end.0, delta)?;
        Some(Self::new(self.file_id, start, end))
    }
}

/// Source anchor for a substring such as an SFC `<template>` block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAnchor {
    /// Original file that owns the block.
    pub file_id: FileId,
    /// Byte offset where the block content starts in the original file.
    pub base_offset: BytePos,
    /// Length of the block content in bytes.
    pub len: usize,
}

impl SourceAnchor {
    /// Creates a source anchor from an original file id, base offset, and length.
    pub const fn new(file_id: FileId, base_offset: usize, len: usize) -> Self {
        Self {
            file_id,
            base_offset: BytePos(base_offset),
            len,
        }
    }

    /// Returns the full source span covered by this anchor.
    pub fn full_span(self) -> Span {
        Span::new(
            self.file_id,
            self.base_offset.0,
            self.base_offset.0 + self.len,
        )
    }

    /// Converts a local byte offset inside the anchored block to an original-file offset.
    pub fn absolute_offset(self, local: BytePos) -> Option<BytePos> {
        if local.0 > self.len {
            return None;
        }
        Some(BytePos(self.base_offset.0 + local.0))
    }

    /// Converts an original-file offset back to a local byte offset inside the block.
    pub fn local_offset(self, absolute: BytePos) -> Option<BytePos> {
        if absolute.0 < self.base_offset.0 || absolute.0 > self.base_offset.0 + self.len {
            return None;
        }
        Some(BytePos(absolute.0 - self.base_offset.0))
    }

    /// Creates an original-file span from local block byte offsets.
    pub fn span(self, start: usize, end: usize) -> Option<Span> {
        if start > end || end > self.len {
            return None;
        }
        Some(Span::new(
            self.file_id,
            self.base_offset.0 + start,
            self.base_offset.0 + end,
        ))
    }
}

/// Zero-based generated position in a source map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GeneratedPosition {
    /// Zero-based generated line.
    pub line: u32,
    /// Zero-based generated UTF-16 column.
    pub column: u32,
}

impl GeneratedPosition {
    /// Creates a generated source-map position.
    pub const fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

/// One generated-to-original source-map segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMapEntry {
    /// Generated position where this mapping starts.
    pub generated: GeneratedPosition,
    /// Original source span referenced by this mapping.
    pub original: Span,
}

/// Original source position resolved from a source-map query.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMappedPosition {
    /// Original source span referenced by the matched segment.
    pub span: Span,
    /// Original one-based source location for `span.start`.
    pub loc: Loc,
}

/// Queryable source-map trace built from normalized generated-to-original entries.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMapTrace {
    entries: Vec<SourceMapEntry>,
}

impl SourceMapTrace {
    /// Creates a trace from unsorted entries.
    pub fn new(mut entries: Vec<SourceMapEntry>) -> Self {
        entries.sort_by_key(|entry| (entry.generated.line, entry.generated.column));
        Self { entries }
    }

    /// Adds a mapping entry while keeping generated positions queryable.
    pub fn add_mapping(&mut self, generated: GeneratedPosition, original: Span) {
        self.entries.push(SourceMapEntry {
            generated,
            original,
        });
        self.entries
            .sort_by_key(|entry| (entry.generated.line, entry.generated.column));
    }

    /// Returns all normalized entries in generated-position order.
    pub fn entries(&self) -> &[SourceMapEntry] {
        &self.entries
    }

    /// Resolves a generated position to the nearest preceding original span on the same line.
    pub fn original_span_at(&self, generated: GeneratedPosition) -> Option<Span> {
        self.entries
            .iter()
            .rev()
            .find(|entry| {
                entry.generated.line == generated.line && entry.generated.column <= generated.column
            })
            .map(|entry| entry.original)
    }

    /// Resolves a generated position to an original location using a source map.
    pub fn original_position_at(
        &self,
        sources: &SourceMap,
        generated: GeneratedPosition,
    ) -> Option<SourceMappedPosition> {
        let span = self.original_span_at(generated)?;
        let loc = sources.loc_at(span.file_id, span.start)?;
        Some(SourceMappedPosition { span, loc })
    }
}

/// Source text plus cached line-start offsets for location projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// File id assigned by the owning `SourceMap`.
    pub id: FileId,
    /// Optional display path for diagnostics and source maps.
    pub path: Option<PathBuf>,
    /// Original source text.
    pub text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    /// Creates a source file and computes its line-start table.
    pub fn new(id: FileId, path: Option<PathBuf>, text: impl Into<String>) -> Self {
        let text = text.into();
        let line_starts = compute_line_starts(&text);
        Self {
            id,
            path,
            text,
            line_starts,
        }
    }

    /// Returns the source text length in bytes.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns `true` when this file contains no source text.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Returns the number of source lines, including a trailing empty line.
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Returns the byte offset where a one-based line starts.
    pub fn line_start(&self, line: usize) -> Option<BytePos> {
        self.line_starts
            .get(line.checked_sub(1)?)
            .copied()
            .map(BytePos)
    }

    /// Returns a single source line without its newline terminator.
    pub fn line_text(&self, line: usize) -> Option<&str> {
        let (start, end) = self.line_bounds_without_newline(line)?;
        self.text.get(start..end)
    }

    /// Converts a byte position to a one-based line and UTF-16 column.
    pub fn loc_at(&self, pos: BytePos) -> Option<Loc> {
        let offset = pos.0;
        if offset > self.text.len() || !self.text.is_char_boundary(offset) {
            return None;
        }
        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(index) => index,
            Err(0) => 0,
            Err(index) => index - 1,
        };
        let line_start = self.line_starts[line_index];
        let column = self.text[line_start..offset].encode_utf16().count() + 1;
        Some(Loc {
            line: line_index + 1,
            column,
        })
    }

    /// Converts a one-based line and UTF-16 column to a byte position.
    pub fn byte_pos_at(&self, loc: Loc) -> Option<BytePos> {
        if loc.line == 0 || loc.column == 0 {
            return None;
        }
        let (line_start, line_end) = self.line_bounds_without_newline(loc.line)?;
        let target_column = loc.column - 1;
        let mut current_column = 0usize;
        if target_column == 0 {
            return Some(BytePos(line_start));
        }
        for (offset, ch) in self.text[line_start..line_end].char_indices() {
            if current_column == target_column {
                return Some(BytePos(line_start + offset));
            }
            current_column += ch.len_utf16();
            if current_column == target_column {
                return Some(BytePos(line_start + offset + ch.len_utf8()));
            }
            if current_column > target_column {
                return None;
            }
        }
        if current_column == target_column {
            Some(BytePos(line_end))
        } else {
            None
        }
    }

    /// Returns a UTF-8 boundary-checked slice for the given byte range.
    pub fn slice(&self, start: BytePos, end: BytePos) -> Option<&str> {
        if start.0 > end.0 || end.0 > self.text.len() {
            return None;
        }
        if !self.text.is_char_boundary(start.0) || !self.text.is_char_boundary(end.0) {
            return None;
        }
        Some(&self.text[start.0..end.0])
    }

    /// Returns a UTF-8 boundary-checked slice for a span in this file.
    pub fn slice_span(&self, span: Span) -> Option<&str> {
        if span.file_id != self.id {
            return None;
        }
        self.slice(span.start, span.end)
    }

    /// Converts a span in this file to start and end source locations.
    pub fn span_locs(&self, span: Span) -> Option<LocSpan> {
        if span.file_id != self.id {
            return None;
        }
        Some(LocSpan {
            start: self.loc_at(span.start)?,
            end: self.loc_at(span.end)?,
        })
    }

    /// Creates a source anchor for a substring of this file.
    pub fn source_anchor(&self, base_offset: usize, len: usize) -> Option<SourceAnchor> {
        self.slice(BytePos(base_offset), BytePos(base_offset.checked_add(len)?))?;
        Some(SourceAnchor::new(self.id, base_offset, len))
    }

    /// Renders a single-line code frame for the given byte range.
    pub fn code_frame(
        &self,
        start: BytePos,
        end: BytePos,
        message: Option<&str>,
    ) -> Option<String> {
        let start_loc = self.loc_at(start)?;
        let end_loc = self.loc_at(end).unwrap_or(start_loc);
        let line = self.line_text(start_loc.line)?.to_string();
        let caret_width = if end_loc.line == start_loc.line {
            end_loc.column.saturating_sub(start_loc.column).max(1)
        } else {
            line.encode_utf16()
                .count()
                .saturating_sub(start_loc.column.saturating_sub(1))
                .max(1)
        };

        let mut frame = String::new();
        if let Some(message) = message {
            frame.push_str(message);
            frame.push('\n');
        }
        frame.push_str(&format!("{:>4} | {}\n", start_loc.line, line));
        frame.push_str(&format!(
            "     | {}{}\n",
            " ".repeat(start_loc.column.saturating_sub(1)),
            "^".repeat(caret_width)
        ));
        Some(frame)
    }

    fn line_bounds_without_newline(&self, line: usize) -> Option<(usize, usize)> {
        let index = line.checked_sub(1)?;
        let start = *self.line_starts.get(index)?;
        let next_start = self
            .line_starts
            .get(index + 1)
            .copied()
            .unwrap_or(self.text.len());
        let mut end = next_start;
        let bytes = self.text.as_bytes();
        if end > start && bytes.get(end - 1) == Some(&b'\n') {
            end -= 1;
            if end > start && bytes.get(end - 1) == Some(&b'\r') {
                end -= 1;
            }
        } else if end > start && bytes.get(end - 1) == Some(&b'\r') {
            end -= 1;
        }
        Some((start, end))
    }
}

/// Collection of source files used for span lookup and diagnostic rendering.
#[derive(Default, Clone, Debug)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    /// Adds a file and returns its assigned id.
    pub fn add_file(&mut self, path: Option<PathBuf>, text: impl Into<String>) -> FileId {
        let id = FileId(self.files.len() as u32);
        self.files.push(SourceFile::new(id, path, text));
        id
    }

    /// Returns a registered source file by id.
    pub fn file(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.0 as usize)
    }

    /// Converts a byte position in a registered file to a source location.
    pub fn loc_at(&self, id: FileId, pos: BytePos) -> Option<Loc> {
        self.file(id)?.loc_at(pos)
    }

    /// Converts a one-based line and UTF-16 column to a byte position.
    pub fn byte_pos_at(&self, id: FileId, loc: Loc) -> Option<BytePos> {
        self.file(id)?.byte_pos_at(loc)
    }

    /// Returns a UTF-8 boundary-checked slice in a registered file.
    pub fn slice(&self, id: FileId, start: BytePos, end: BytePos) -> Option<&str> {
        self.file(id)?.slice(start, end)
    }

    /// Returns a UTF-8 boundary-checked slice for a span.
    pub fn slice_span(&self, span: Span) -> Option<&str> {
        self.file(span.file_id)?.slice_span(span)
    }

    /// Converts a span to source locations.
    pub fn span_locs(&self, span: Span) -> Option<LocSpan> {
        self.file(span.file_id)?.span_locs(span)
    }

    /// Creates a source anchor for an SFC block or other source substring.
    pub fn source_anchor(
        &self,
        id: FileId,
        base_offset: usize,
        len: usize,
    ) -> Option<SourceAnchor> {
        self.file(id)?.source_anchor(base_offset, len)
    }

    /// Renders a code frame for a byte range in a registered file.
    pub fn code_frame(
        &self,
        id: FileId,
        start: BytePos,
        end: BytePos,
        message: Option<&str>,
    ) -> Option<String> {
        self.file(id)?.code_frame(start, end, message)
    }
}

fn compute_line_starts(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut starts = vec![0];
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                if index + 1 < bytes.len() && bytes[index + 1] == b'\n' {
                    starts.push(index + 2);
                    index += 2;
                } else {
                    starts.push(index + 1);
                    index += 1;
                }
            }
            b'\n' => {
                starts.push(index + 1);
                index += 1;
            }
            _ => index += 1,
        }
    }

    starts
}

fn translate_offset(offset: usize, delta: isize) -> Option<usize> {
    if delta.is_negative() {
        offset.checked_sub(delta.unsigned_abs())
    } else {
        offset.checked_add(delta as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loc_tracks_lf_and_crlf() {
        let file = SourceFile::new(FileId(0), None, "a\nb\r\nc");
        assert_eq!(file.loc_at(BytePos(0)), Some(Loc { line: 1, column: 1 }));
        assert_eq!(file.loc_at(BytePos(2)), Some(Loc { line: 2, column: 1 }));
        assert_eq!(file.loc_at(BytePos(5)), Some(Loc { line: 3, column: 1 }));
    }

    #[test]
    fn loc_tracks_mixed_newlines() {
        let file = SourceFile::new(FileId(0), None, "a\rb\nc\r\nd");
        assert_eq!(file.line_count(), 4);
        assert_eq!(file.loc_at(BytePos(2)), Some(Loc { line: 2, column: 1 }));
        assert_eq!(file.loc_at(BytePos(4)), Some(Loc { line: 3, column: 1 }));
        assert_eq!(file.loc_at(BytePos(7)), Some(Loc { line: 4, column: 1 }));
        assert_eq!(file.line_text(1), Some("a"));
        assert_eq!(file.line_text(4), Some("d"));
    }

    #[test]
    fn loc_counts_utf16_columns_and_roundtrips() {
        let file = SourceFile::new(FileId(0), None, "a中😀b");
        assert_eq!(file.loc_at(BytePos(0)), Some(Loc { line: 1, column: 1 }));
        assert_eq!(file.loc_at(BytePos(1)), Some(Loc { line: 1, column: 2 }));
        assert_eq!(
            file.loc_at(BytePos("a中".len())),
            Some(Loc { line: 1, column: 3 })
        );
        assert_eq!(
            file.loc_at(BytePos("a中😀".len())),
            Some(Loc { line: 1, column: 5 })
        );
        assert_eq!(
            file.byte_pos_at(Loc { line: 1, column: 5 }),
            Some(BytePos("a中😀".len()))
        );
        assert_eq!(file.byte_pos_at(Loc { line: 1, column: 4 }), None);
    }

    #[test]
    fn code_frame_renders() {
        let file = SourceFile::new(FileId(0), None, "hello\nworld");
        let frame = file
            .code_frame(BytePos(6), BytePos(11), Some("oops"))
            .expect("frame");
        assert!(frame.contains("oops"));
        assert!(frame.contains("world"));
        assert!(frame.contains("^"));
    }

    #[test]
    fn slice_span_checks_file_identity() {
        let file = SourceFile::new(FileId(2), None, "hello");
        assert_eq!(file.slice_span(Span::new(FileId(2), 1, 4)), Some("ell"));
        assert_eq!(file.slice_span(Span::new(FileId(1), 1, 4)), None);
    }

    #[test]
    fn source_anchor_maps_sfc_block_offsets_to_original_locations() {
        let source = "<template>\r\n  <div>中😀</div>\r\n</template>";
        let mut sources = SourceMap::default();
        let file_id = sources.add_file(Some("App.vue".into()), source);
        let block_start = source.find("  <div>").unwrap();
        let block_end = source.find("\r\n</template>").unwrap();
        let block = &source[block_start..block_end];
        let anchor = sources
            .source_anchor(file_id, block_start, block.len())
            .expect("anchor");
        let local_start = block.find('中').unwrap();
        let local_end = block.find("</div>").unwrap();
        let span = anchor.span(local_start, local_end).expect("span");

        assert_eq!(sources.slice_span(span), Some("中😀"));
        assert_eq!(
            sources.span_locs(span),
            Some(LocSpan {
                start: Loc { line: 2, column: 8 },
                end: Loc {
                    line: 2,
                    column: 11
                }
            })
        );
        assert_eq!(anchor.local_offset(span.start), Some(BytePos(local_start)));
    }

    #[test]
    fn source_map_trace_resolves_generated_position_to_original_location() {
        let mut sources = SourceMap::default();
        let file_id = sources.add_file(Some("Comp.vue".into()), "<template>{{ msg }}</template>");
        let original = Span::new(file_id, 13, 16);
        let trace = SourceMapTrace::new(vec![SourceMapEntry {
            generated: GeneratedPosition::new(0, 18),
            original,
        }]);

        assert_eq!(
            trace.original_span_at(GeneratedPosition::new(0, 20)),
            Some(original)
        );
        assert_eq!(
            trace.original_position_at(&sources, GeneratedPosition::new(0, 20)),
            Some(SourceMappedPosition {
                span: original,
                loc: Loc {
                    line: 1,
                    column: 14
                }
            })
        );
        assert_eq!(trace.original_span_at(GeneratedPosition::new(1, 0)), None);
    }
}
