//! ### SVN 工具函数
//!

use std::{fmt::Display, io};

use crossterm::execute;

use super::error::{AppResult, AppError};

#[derive(Debug, PartialEq, PartialOrd)]
pub enum Revision {
    Head,
    Number(u64),
}

impl Display for Revision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Revision::Head => write!(f, "HEAD"),
            Revision::Number(n) => write!(f, "r{}", n),
        }
    }
}

pub fn parse_revision_arg(input: &str) -> AppResult<Revision> {
    let s = input.trim();
    
    // 特殊处理 HEAD
    if s.eq_ignore_ascii_case("HEAD") {
        return Ok(Revision::Head);
    }

    let target_rev: u64 = match s.trim_start_matches(|c| c == 'r' || c == 'R').parse() {
        Ok(r) => r,
        Err(_) => {
            return Err(AppError::RevisionParse(input.to_string()));
        }
    };

    Ok(Revision::Number(target_rev))
}

pub fn auto_decode(input: &[u8]) -> AppResult<String> {
    // First, try UTF-8, which is the most common.
    if let Ok(s) = String::from_utf8(input.to_vec()) {
        return Ok(s.trim().to_string());
    }

    // Fallback to chardetng for other encodings if UTF-8 fails.
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(input, true);
    let encoding = detector.guess(None, true);
    let (decoded_bytes, _, had_errors) = encoding.decode(input);

    if had_errors {
        // Even the fallback had errors, so we return an error.
        // We use the From<FromUtf8Error> trait we defined earlier.
        return Err(String::from_utf8(vec![]).unwrap_err().into());
    }
    
    Ok(decoded_bytes.trim().to_string())
}

pub struct CursorGuard;

impl CursorGuard {
    pub fn new() -> Self {
        execute!(io::stdout(), crossterm::cursor::Hide).ok();
        execute!(io::stderr(), crossterm::cursor::Hide).ok();
        CursorGuard
    }
}

impl Drop for CursorGuard {
    fn drop(&mut self) {
        execute!(io::stdout(), crossterm::cursor::Show).ok();
        execute!(io::stderr(), crossterm::cursor::Show).ok();
    }
}


