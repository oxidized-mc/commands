//! Cursor-based string reader for argument parsing.

use crate::CommandError;
use crate::arguments::StringKind;

/// A simple cursor-based string reader for argument parsing.
pub struct StringReader<'a> {
    input: &'a str,
    cursor: usize,
}

impl<'a> StringReader<'a> {
    /// Creates a new reader at the given start position.
    pub fn new(input: &'a str, cursor: usize) -> Self {
        Self { input, cursor }
    }

    /// Returns the current cursor position.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Advances the cursor by `n` bytes.
    pub fn advance(&mut self, n: usize) {
        self.cursor += n;
    }

    /// Returns the remaining unparsed input.
    pub fn remaining(&self) -> &str {
        &self.input[self.cursor..]
    }

    /// Returns `true` if there is more input.
    pub fn can_read(&self) -> bool {
        self.cursor < self.input.len()
    }

    /// Peeks at the next character without consuming.
    pub fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    /// Skips whitespace.
    pub fn skip_whitespace(&mut self) {
        while self.can_read() && self.peek() == Some(' ') {
            self.cursor += 1;
        }
    }

    /// Reads a single word (no spaces).
    pub fn read_word(&mut self) -> &str {
        let start = self.cursor;
        while self.can_read() && self.peek() != Some(' ') {
            self.cursor += 1;
        }
        &self.input[start..self.cursor]
    }

    /// Reads and parses a numeric value from the next word.
    fn read_numeric<T: std::str::FromStr>(&mut self, type_name: &str) -> Result<T, CommandError> {
        let word = self.read_word();
        word.parse::<T>()
            .map_err(|_| CommandError::Parse(format!("Expected {type_name}, got '{word}'")))
    }

    /// Reads an integer.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::Parse`] if the next word is not a valid integer.
    pub fn read_integer(&mut self) -> Result<i32, CommandError> {
        self.read_numeric("integer")
    }

    /// Reads a long.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::Parse`] if the next word is not a valid long.
    pub fn read_long(&mut self) -> Result<i64, CommandError> {
        self.read_numeric("long")
    }

    /// Reads a float.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::Parse`] if the next word is not a valid float.
    pub fn read_float(&mut self) -> Result<f32, CommandError> {
        self.read_numeric("float")
    }

    /// Reads a double.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::Parse`] if the next word is not a valid double.
    pub fn read_double(&mut self) -> Result<f64, CommandError> {
        self.read_numeric("double")
    }

    /// Reads a boolean.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::Parse`] if the next word is not `"true"` or
    /// `"false"`.
    pub fn read_bool(&mut self) -> Result<bool, CommandError> {
        let word = self.read_word();
        match word {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(CommandError::Parse(format!(
                "Expected 'true' or 'false', got '{word}'"
            ))),
        }
    }

    /// Reads a string according to the given kind.
    pub fn read_string(&mut self, kind: StringKind) -> String {
        match kind {
            StringKind::SingleWord => self.read_word().to_string(),
            StringKind::GreedyPhrase => {
                let rest = self.remaining().to_string();
                self.cursor = self.input.len();
                rest
            },
            StringKind::QuotablePhrase => {
                if self.peek() == Some('"') {
                    self.cursor += 1; // skip opening quote
                    let mut result = String::new();
                    while self.can_read() {
                        let ch = self.input.as_bytes()[self.cursor] as char;
                        if ch == '\\' && self.cursor + 1 < self.input.len() {
                            let next = self.input.as_bytes()[self.cursor + 1] as char;
                            if next == '"' || next == '\\' {
                                result.push(next);
                                self.cursor += 2;
                                continue;
                            }
                        }
                        if ch == '"' {
                            break;
                        }
                        result.push(ch);
                        self.cursor += 1;
                    }
                    if self.can_read() {
                        self.cursor += 1; // skip closing quote
                    }
                    result
                } else {
                    self.read_word().to_string()
                }
            },
        }
    }
}
