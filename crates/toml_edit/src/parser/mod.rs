#![allow(clippy::type_complexity)]

use std::cell::RefCell;
pub(crate) mod array;
pub(crate) mod datetime;
pub(crate) mod document;
pub(crate) mod error;
pub(crate) mod inline_table;
pub(crate) mod key;
pub(crate) mod numbers;
pub(crate) mod state;
pub(crate) mod strings;
pub(crate) mod table;
pub(crate) mod trivia;
pub(crate) mod value;

pub(crate) use crate::error::TomlError;

pub(crate) fn parse_document<S: AsRef<str>>(raw: S) -> Result<crate::ImDocument<S>, TomlError> {
    use prelude::*;

    let b = new_input(raw.as_ref());
    let state = RefCell::new(state::ParseState::new());
    let state_ref = &state;
    document::document(state_ref)
        .parse(b)
        .map_err(|e| TomlError::new(e, b))?;
    let doc = state
        .into_inner()
        .into_document(raw)
        .map_err(|e| TomlError::custom(e.to_string(), None))?;
    Ok(doc)
}

pub(crate) fn parse_key(raw: &str) -> Result<crate::Key, TomlError> {
    use prelude::*;

    let b = new_input(raw);
    let result = key::simple_key.parse(b);
    match result {
        Ok((raw, key)) => {
            Ok(crate::Key::new(key).with_repr_unchecked(crate::Repr::new_unchecked(raw)))
        }
        Err(e) => Err(TomlError::new(e, b)),
    }
}

pub(crate) fn parse_key_path(raw: &str) -> Result<Vec<crate::Key>, TomlError> {
    use prelude::*;

    let b = new_input(raw);
    let result = key::key.parse(b);
    match result {
        Ok(mut keys) => {
            for key in &mut keys {
                key.despan(raw);
            }
            Ok(keys)
        }
        Err(e) => Err(TomlError::new(e, b)),
    }
}

pub(crate) fn parse_value(raw: &str) -> Result<crate::Value, TomlError> {
    use prelude::*;

    let b = new_input(raw);
    let parsed = value::value(RecursionCheck::default()).parse(b);
    match parsed {
        Ok(mut value) => {
            // Only take the repr and not decor, as its probably not intended
            value.decor_mut().clear();
            value.despan(raw);
            Ok(value)
        }
        Err(e) => Err(TomlError::new(e, b)),
    }
}

pub(crate) mod prelude {
    pub(crate) use winnow::combinator::dispatch;
    pub(crate) use winnow::error::ContextError;
    pub(crate) use winnow::error::FromExternalError;
    pub(crate) use winnow::error::StrContext;
    pub(crate) use winnow::error::StrContextValue;
    pub(crate) use winnow::PResult;
    pub(crate) use winnow::Parser;

    pub(crate) type Input<'b> = winnow::Located<&'b winnow::BStr>;

    pub(crate) fn new_input(s: &str) -> Input<'_> {
        winnow::Located::new(winnow::BStr::new(s))
    }

    #[cfg(not(feature = "unbounded"))]
    #[derive(Copy, Clone, Debug, Default)]
    pub(crate) struct RecursionCheck {
        current: usize,
    }

    #[cfg(not(feature = "unbounded"))]
    const LIMIT: usize = 80;

    #[cfg(not(feature = "unbounded"))]
    impl RecursionCheck {
        pub(crate) fn check_depth(depth: usize) -> Result<(), super::error::CustomError> {
            if depth < LIMIT {
                Ok(())
            } else {
                Err(super::error::CustomError::RecursionLimitExceeded)
            }
        }

        pub(crate) fn recursing(
            mut self,
            input: &mut Input<'_>,
        ) -> Result<Self, winnow::error::ErrMode<ContextError>> {
            self.current += 1;
            if self.current < LIMIT {
                Ok(self)
            } else {
                Err(winnow::error::ErrMode::from_external_error(
                    input,
                    winnow::error::ErrorKind::Eof,
                    super::error::CustomError::RecursionLimitExceeded,
                ))
            }
        }
    }

    #[cfg(feature = "unbounded")]
    #[derive(Copy, Clone, Debug, Default)]
    pub(crate) struct RecursionCheck {}

    #[cfg(feature = "unbounded")]
    impl RecursionCheck {
        pub(crate) fn check_depth(_depth: usize) -> Result<(), super::error::CustomError> {
            Ok(())
        }

        pub(crate) fn recursing(
            self,
            _input: &mut Input<'_>,
        ) -> Result<Self, winnow::error::ErrMode<ContextError>> {
            Ok(self)
        }
    }
}

#[cfg(test)]
#[cfg(feature = "parse")]
#[cfg(feature = "display")]
mod test {
    use super::*;

    #[test]
    fn documents() {
        let documents = [
            "",
            r#"
# This is a TOML document.

title = "TOML Example"

    [owner]
    name = "Tom Preston-Werner"
    dob = 1979-05-27T07:32:00-08:00 # First class dates

    [database]
    server = "192.168.1.1"
    ports = [ 8001, 8001, 8002 ]
    connection_max = 5000
    enabled = true

    [servers]

    # Indentation (tabs and/or spaces) is allowed but not required
[servers.alpha]
    ip = "10.0.0.1"
    dc = "eqdc10"

    [servers.beta]
    ip = "10.0.0.2"
    dc = "eqdc10"

    [clients]
    data = [ ["gamma", "delta"], [1, 2] ]

    # Line breaks are OK when inside arrays
hosts = [
    "alpha",
    "omega"
]

   'some.weird .stuff'   =  """
                         like
                         that
                      #   """ # this broke my syntax highlighting
   " also. like " = '''
that
'''
   double = 2e39 # this number looks familiar
# trailing comment"#,
            r#""#,
            r#"  "#,
            r#" hello = 'darkness' # my old friend
"#,
            r#"[parent . child]
key = "value"
"#,
            r#"hello.world = "a"
"#,
            r#"foo = 1979-05-27 # Comment
"#,
        ];
        for input in documents {
            dbg!(input);
            let parsed = parse_document(input).map(|d| d.into_mut());
            let doc = match parsed {
                Ok(doc) => doc,
                Err(err) => {
                    panic!(
                        "Parse error: {:?}\nFailed to parse:\n```\n{}\n```",
                        err, input
                    )
                }
            };

            snapbox::assert_eq(input, doc.to_string());
        }
    }

    #[test]
    fn documents_parse_only() {
        let parse_only = ["\u{FEFF}
[package]
name = \"foo\"
version = \"0.0.1\"
authors = []
"];
        for input in parse_only {
            dbg!(input);
            let parsed = parse_document(input).map(|d| d.into_mut());
            match parsed {
                Ok(_) => (),
                Err(err) => {
                    panic!(
                        "Parse error: {:?}\nFailed to parse:\n```\n{}\n```",
                        err, input
                    )
                }
            }
        }
    }

    #[test]
    fn invalid_documents() {
        let invalid_inputs = [r#" hello = 'darkness' # my old friend
$"#];
        for input in invalid_inputs {
            dbg!(input);
            let parsed = parse_document(input).map(|d| d.into_mut());
            assert!(parsed.is_err(), "Input: {:?}", input);
        }
    }
}
