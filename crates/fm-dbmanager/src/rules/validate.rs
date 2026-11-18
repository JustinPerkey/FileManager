//! `validate`: a rule plus an input becomes the value to store, or issues.

use regex::Regex;

use super::duration::{self, DurationError};
use super::{FieldIssue, FieldIssueCode, FieldKind, FieldRule};
use crate::value::DbValue;

fn issue(code: FieldIssueCode, message: impl Into<String>, limit: Option<String>) -> FieldIssue {
    FieldIssue {
        code,
        message: message.into(),
        limit,
    }
}

fn one(code: FieldIssueCode, message: &str) -> Vec<FieldIssue> {
    vec![issue(code, message, None)]
}

/// Checks `input` against `rule`.
///
/// # Errors
///
/// The issues found, in the order of [`FieldIssueCode`]'s variants. Never
/// returns a database-dependent code.
pub fn validate(rule: &FieldRule, input: &DbValue) -> Result<DbValue, Vec<FieldIssue>> {
    if matches!(input, DbValue::Null) {
        return if rule.nullable {
            Ok(DbValue::Null)
        } else {
            Err(one(FieldIssueCode::Required, "A value is required."))
        };
    }
    let wrong = || {
        Err(one(
            FieldIssueCode::WrongType,
            "The value has the wrong type.",
        ))
    };
    match (rule.kind, input) {
        (FieldKind::Integer, DbValue::Integer(s)) => integer(rule, s),
        (FieldKind::Real, DbValue::Real(s)) => real(rule, s),
        (FieldKind::Bool, DbValue::Integer(s)) => boolean(s),
        (FieldKind::Duration, DbValue::Duration(s)) => duration_value(rule, s),
        (FieldKind::Text, DbValue::Text(s)) => text(rule, s),
        _ => wrong(),
    }
}

fn is_integer_grammar(s: &str) -> bool {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

fn canonical_integer(s: &str) -> Option<i64> {
    if is_integer_grammar(s) {
        s.parse().ok()
    } else {
        None
    }
}

fn is_real_grammar(s: &str) -> bool {
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    let (mantissa, exponent) = match s.find(['e', 'E']) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    if let Some(exp) = exponent {
        let digits = exp.strip_prefix(['+', '-']).unwrap_or(exp);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    let (whole, frac) = match mantissa.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (mantissa, None),
    };
    let all_digits = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
    if !all_digits(whole) || !frac.is_none_or(all_digits) {
        return false;
    }
    // `5`, `5.`, `5.5` or `.5`; never `.` alone.
    !whole.is_empty() || frac.is_some_and(|f| !f.is_empty())
}

fn bounds<T: PartialOrd + Copy>(
    value: T,
    rule: &FieldRule,
    parse: impl Fn(&str) -> Option<T>,
    out: &mut Vec<FieldIssue>,
) {
    if let Some(min) = rule.min.as_deref() {
        if parse(min).is_some_and(|m| value < m) {
            out.push(issue(
                FieldIssueCode::BelowMin,
                format!("The value is below the minimum of {min}."),
                Some(min.to_string()),
            ));
        }
    }
    if let Some(max) = rule.max.as_deref() {
        if parse(max).is_some_and(|m| value > m) {
            out.push(issue(
                FieldIssueCode::AboveMax,
                format!("The value is above the maximum of {max}."),
                Some(max.to_string()),
            ));
        }
    }
}

fn integer(rule: &FieldRule, s: &str) -> Result<DbValue, Vec<FieldIssue>> {
    let Some(n) = canonical_integer(s) else {
        return Err(one(
            FieldIssueCode::NotInteger,
            "The value is not a whole number that fits.",
        ));
    };
    let mut issues = Vec::new();
    bounds(n, rule, canonical_integer, &mut issues);
    if let Some(allowed) = &rule.allowed {
        let found = allowed.iter().any(|a| match &a.value {
            DbValue::Integer(v) => canonical_integer(v) == Some(n),
            _ => false,
        });
        if !found {
            issues.push(issue(
                FieldIssueCode::NotAllowed,
                "The value is not one of the allowed values.",
                None,
            ));
        }
    }
    if issues.is_empty() {
        Ok(DbValue::Integer(n.to_string()))
    } else {
        Err(issues)
    }
}

fn parse_real(s: &str) -> Option<f64> {
    if !is_real_grammar(s) {
        return None;
    }
    s.parse::<f64>().ok().filter(|x| x.is_finite())
}

fn real(rule: &FieldRule, s: &str) -> Result<DbValue, Vec<FieldIssue>> {
    let Some(x) = parse_real(s) else {
        return Err(one(
            FieldIssueCode::NotReal,
            "The value is not a finite number.",
        ));
    };
    let mut issues = Vec::new();
    bounds(x, rule, parse_real, &mut issues);
    if issues.is_empty() {
        Ok(DbValue::Real(s.to_string()))
    } else {
        Err(issues)
    }
}

fn boolean(s: &str) -> Result<DbValue, Vec<FieldIssue>> {
    match canonical_integer(s) {
        Some(0) => Ok(DbValue::Integer("0".into())),
        Some(1) => Ok(DbValue::Integer("1".into())),
        _ => Err(one(FieldIssueCode::NotBool, "The value must be 0 or 1.")),
    }
}

fn duration_value(rule: &FieldRule, s: &str) -> Result<DbValue, Vec<FieldIssue>> {
    let Some(unit) = rule.unit else {
        return Err(one(
            FieldIssueCode::NotDuration,
            "The field has no duration unit.",
        ));
    };
    let count = match duration::parse(s, unit) {
        Ok(count) => count,
        Err(DurationError::Precision) => {
            return Err(vec![issue(
                FieldIssueCode::DurationPrecision,
                format!("The duration must be a whole number of {}.", unit.name()),
                Some(unit.name().to_string()),
            )]);
        }
        Err(DurationError::Grammar | DurationError::Overflow) => {
            return Err(one(
                FieldIssueCode::NotDuration,
                "The value is not a valid duration.",
            ));
        }
    };
    let mut issues = Vec::new();
    bounds(count, rule, |b| duration::parse(b, unit).ok(), &mut issues);
    if issues.is_empty() {
        Ok(DbValue::Duration(duration::format(count, unit)))
    } else {
        Err(issues)
    }
}

fn text(rule: &FieldRule, s: &str) -> Result<DbValue, Vec<FieldIssue>> {
    if rule.empty_is_null && rule.nullable && s.is_empty() {
        return Ok(DbValue::Null);
    }
    let mut issues = Vec::new();
    let length = s.chars().count();
    if let Some(min) = rule.min_length {
        if length < min as usize {
            issues.push(issue(
                FieldIssueCode::TooShort,
                format!("The text must be at least {min} characters."),
                Some(min.to_string()),
            ));
        }
    }
    if let Some(max) = rule.max_length {
        if length > max as usize {
            issues.push(issue(
                FieldIssueCode::TooLong,
                format!("The text must be at most {max} characters."),
                Some(max.to_string()),
            ));
        }
    }
    if let Some(pattern) = &rule.pattern {
        // A pattern that does not compile matches nothing; resolving a rule
        // checks patterns up front.
        let matches = Regex::new(&format!("^(?:{pattern})$")).is_ok_and(|re| re.is_match(s));
        if !matches {
            issues.push(issue(
                FieldIssueCode::PatternMismatch,
                "The text does not match the required pattern.",
                Some(pattern.clone()),
            ));
        }
    }
    if let Some(allowed) = &rule.allowed {
        let found = allowed
            .iter()
            .any(|a| matches!(&a.value, DbValue::Text(v) if v == s));
        if !found {
            issues.push(issue(
                FieldIssueCode::NotAllowed,
                "The text is not one of the allowed values.",
                None,
            ));
        }
    }
    if issues.is_empty() {
        Ok(DbValue::Text(s.to_string()))
    } else {
        Err(issues)
    }
}
