use std::fmt::Display;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Identifier(Vec<Rule>);

#[derive(Debug, Clone)]
pub struct ParseError(String);

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

impl Identifier {
    pub fn complexity(&self) -> usize {
        self.0.len()
    }

    pub fn parse(s: &str) -> Result<Self, ParseError> {
        let ids = s.split(',');
        let mut parsed_ids = Vec::new();
        for (n, id) in ids.enumerate() {
            match Rule::parse(id) {
                Ok(parsed_id) => parsed_ids.push(parsed_id),
                Err(err) => {
                    return Err(ParseError(format!(
                        "Error in rule {n} of '{s}': {err}",
                        n = n + 1
                    )));
                }
            }
        }

        Ok(Self(parsed_ids))
    }

    pub fn matches(&self, path: Option<&Path>, head: &[u8]) -> bool {
        for id in &self.0 {
            match id {
                Rule::Ext(ext) => {
                    if let Some(path_ext) =
                        path.and_then(|x| x.extension()).and_then(|x| x.to_str())
                    {
                        if path_ext != ext {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                Rule::MagicBytes(mb) => {
                    if !head
                        .get(mb.pos..)
                        .map_or(false, |s| s.starts_with(&mb.bytes))
                    {
                        return false;
                    }
                }
            }
        }

        true
    }
}

#[derive(Debug, Clone)]
enum Rule {
    MagicBytes(MagicBytes),
    Ext(String),
}

impl Rule {
    fn parse(rule: &str) -> Result<Self, String> {
        if let Some(ext) = rule.strip_prefix("ext:") {
            Ok(Self::Ext(ext.to_string()))
        } else if let Some((pos, bytes)) = rule.split_once(':') {
            Ok(Self::MagicBytes(MagicBytes::parse(pos, bytes)?))
        } else {
            Err(format!(
                "This identifier has no known prefix: {rule}. Must be 'ext:' or '<byte position>:'."
            ))
        }
    }
}

#[derive(Debug, Clone)]
struct MagicBytes {
    pos: usize,
    bytes: Vec<u8>,
}

impl MagicBytes {
    fn parse(pos: &str, sbytes: &str) -> Result<Self, String> {
        enum State {
            None,
            Backslash,
            Hex1,
            Hex2(char),
        }

        let pos = pos
            .parse()
            .map_err(|err| format!("Failed to parse position: {err}"))?;

        let mut state = State::None;
        let mut bytes = Vec::new();

        for char in sbytes.chars() {
            state = match state {
                State::None => match char {
                    '\\' => State::Backslash,
                    c => {
                        if !c.is_ascii() {
                            return Err(format!("Not an ascii char: {c}"));
                        }
                        bytes.push(c as u8);
                        State::None
                    }
                },
                State::Backslash => match char {
                    '\\' => {
                        bytes.push(b'\\');
                        State::None
                    }
                    c => {
                        if c == 'x' {
                            State::Hex1
                        } else {
                            return Err(format!("Escape sequence \\{c} not supported"));
                        }
                    }
                },
                State::Hex1 => State::Hex2(char),
                State::Hex2(c1) => {
                    let s = format!("{c1}{char}");
                    bytes.push(
                        u8::from_str_radix(&s, 16)
                            .map_err(|err| format!("Invalid hex number '{s}': {err}"))?,
                    );
                    State::None
                }
            }
        }

        Ok(Self { pos, bytes })
    }
}

#[cfg(test)]
mod tests {
    use crate::config::indentifier::{Identifier, Rule};

    #[test]
    fn test_mb_parser() {
        let id = Rule::parse(r"0:\x89PNG\x0D\x0A\x1A\x0A").unwrap();
        let Rule::MagicBytes(mb) = id else { panic!() };
        assert_eq!(mb.pos, 0);
        assert_eq!(mb.bytes, b"\x89PNG\x0D\x0A\x1A\x0A");
    }

    #[test]
    fn test_matches() {
        let identifier = Identifier::parse(r"0:\x89PNG\x0D\x0A\x1A\x0A").unwrap();
        assert!(identifier.matches(None, b"\x89PNG\x0D\x0A\x1A\x0Arandomecontent"));
        assert!(!identifier.matches(None, b"\x89PMG\x0D\x0A\x1A\x0Arandomecontent"));

        let identifier = Identifier::parse(r"0:ab,3:de").unwrap();
        assert!(identifier.matches(None, b"abCde"));
    }
}
