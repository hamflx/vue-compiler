#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FileId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BytePos(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loc {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub file_id: FileId,
    pub start: BytePos,
    pub end: BytePos,
}

impl Span {
    pub const fn new(file_id: FileId, start: usize, end: usize) -> Self {
        Self {
            file_id,
            start: BytePos(start),
            end: BytePos(end),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    pub id: FileId,
    pub path: Option<PathBuf>,
    pub text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
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

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

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

    pub fn slice(&self, start: BytePos, end: BytePos) -> Option<&str> {
        if start.0 > end.0 || end.0 > self.text.len() {
            return None;
        }
        if !self.text.is_char_boundary(start.0) || !self.text.is_char_boundary(end.0) {
            return None;
        }
        Some(&self.text[start.0..end.0])
    }

    pub fn code_frame(&self, start: BytePos, end: BytePos, message: Option<&str>) -> Option<String> {
        let start_loc = self.loc_at(start)?;
        let end_loc = self.loc_at(end).unwrap_or(start_loc);
        let line_start = self.line_starts.get(start_loc.line.saturating_sub(1)).copied()?;
        let line_end = self
            .line_starts
            .get(start_loc.line)
            .copied()
            .unwrap_or(self.text.len());
        let line = self.text[line_start..line_end]
            .trim_end_matches(['\r', '\n'])
            .to_string();
        let caret_width = end_loc
            .column
            .saturating_sub(start_loc.column)
            .max(1);

        let mut frame = String::new();
        if let Some(message) = message {
            frame.push_str(message);
            frame.push('\n');
        }
        frame.push_str(&format!("{:>4} | {}\n", start_loc.line, line));
        frame.push_str(&format!("     | {}{}\n", " ".repeat(start_loc.column.saturating_sub(1)), "^".repeat(caret_width)));
        Some(frame)
    }
}

#[derive(Default, Clone, Debug)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn add_file(&mut self, path: Option<PathBuf>, text: impl Into<String>) -> FileId {
        let id = FileId(self.files.len() as u32);
        self.files.push(SourceFile::new(id, path, text));
        id
    }

    pub fn file(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.0 as usize)
    }

    pub fn loc_at(&self, id: FileId, pos: BytePos) -> Option<Loc> {
        self.file(id)?.loc_at(pos)
    }

    pub fn slice(&self, id: FileId, start: BytePos, end: BytePos) -> Option<&str> {
        self.file(id)?.slice(start, end)
    }

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
        assert_eq!(file.loc_at(BytePos("a😀".len())), Some(Loc { line: 1, column: 4 }));
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
