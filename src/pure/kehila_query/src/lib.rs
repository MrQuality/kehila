//! A typed equality grammar.
//! Supported input: ASCII identifier = single-quoted nonempty value.
//! This is not yet the full isomorphic query/Rhai/Wasm compiler.

#[derive(Debug, PartialEq, Eq)]
pub struct Filter {
    pub field: String,
    pub value: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    MissingEquals,
    InvalidField,
    InvalidValue,
}

/// Parse the complete input; reject trailing clauses and ambiguous quoting.
pub fn parse_filter(input: &str) -> Result<Filter, ParseError> {
    let (field, value) = input.split_once('=').ok_or(ParseError::MissingEquals)?;
    let field = field.trim();
    let mut chars = field.chars();
    if !chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(ParseError::InvalidField);
    }
    let value = value.trim();
    let inner = value
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .ok_or(ParseError::InvalidValue)?;
    if inner.is_empty()
        || inner
            .chars()
            .any(|c| c == '\'' || c == '\\' || c.is_control())
    {
        return Err(ParseError::InvalidValue);
    }
    Ok(Filter {
        field: field.to_owned(),
        value: inner.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_complete_filter() {
        assert_eq!(
            parse_filter(" status = 'In Progress' "),
            Ok(Filter {
                field: "status".into(),
                value: "In Progress".into(),
            })
        );
    }

    #[test]
    fn rejects_invalid_or_unsupported_grammar() {
        for input in [
            "",
            "status",
            "1status = 'Open'",
            "status == 'Open'",
            "status = Open",
            "status = ''",
            "status = 'Open' OR project = 'X'",
            "status = 'Open' trailing",
            "status = 'a\\b'",
            "status = 'a\nb'",
        ] {
            assert!(parse_filter(input).is_err(), "accepted {input:?}");
        }
    }

    #[test]
    fn preserves_unicode_values_and_equals() {
        assert_eq!(parse_filter("_field = 'שלום=x'").unwrap().value, "שלום=x");
    }
}
