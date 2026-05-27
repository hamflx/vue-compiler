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

    /// Renders a single-line code frame for the given byte range.
    pub fn code_frame(
        &self,
        start: BytePos,
        end: BytePos,
        message: Option<&str>,
    ) -> Option<String> {
        let start_loc = self.loc_at(start)?;
        let end_loc = self.loc_at(end).unwrap_or(start_loc);
        let line_start = self
            .line_starts
            .get(start_loc.line.saturating_sub(1))
            .copied()?;
        let line_end = self
            .line_starts
            .get(start_loc.line)
            .copied()
            .unwrap_or(self.text.len());
        let line = self.text[line_start..line_end]
            .trim_end_matches(['\r', '\n'])
            .to_string();
        let caret_width = end_loc.column.saturating_sub(start_loc.column).max(1);

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

    /// Returns a UTF-8 boundary-checked slice in a registered file.
    pub fn slice(&self, id: FileId, start: BytePos, end: BytePos) -> Option<&str> {
        self.file(id)?.slice(start, end)
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
    fn loc_counts_utf16_columns() {
        let file = SourceFile::new(FileId(0), None, "a😀b");
        assert_eq!(file.loc_at(BytePos(0)), Some(Loc { line: 1, column: 1 }));
        assert_eq!(file.loc_at(BytePos(1)), Some(Loc { line: 1, column: 2 }));
        assert_eq!(
            file.loc_at(BytePos("a😀".len())),
            Some(Loc { line: 1, column: 4 })
        );
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
}
