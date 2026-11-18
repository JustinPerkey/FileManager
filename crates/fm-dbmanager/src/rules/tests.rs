use serde::Deserialize;

use super::duration::{self, DurationError};
use super::{validate, FieldIssueCode, FieldKind, FieldRule};
use crate::config::DurationUnit;
use crate::value::DbValue;

#[derive(Deserialize)]
struct Case {
    name: String,
    rule: FieldRule,
    input: DbValue,
    ok: Option<DbValue>,
    issues: Vec<FieldIssueCode>,
    limits: Option<Vec<Option<String>>>,
}

#[test]
fn shared_vectors_pass() {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../../examples/dbmanager/validation-cases.json"
    ))
    .expect("vectors parse");
    assert!(cases.len() >= 80, "need at least 80 cases");
    let durations = cases
        .iter()
        .filter(|c| c.rule.kind == FieldKind::Duration)
        .count();
    assert!(durations >= 20, "need at least 20 duration cases");
    for case in cases {
        let got = validate(&case.rule, &case.input);
        match (&case.ok, got) {
            (Some(want), Ok(have)) => {
                assert_eq!(&have, want, "{}", case.name);
                assert!(case.issues.is_empty(), "{}", case.name);
            }
            (None, Err(have)) => {
                let codes: Vec<_> = have.iter().map(|i| i.code).collect();
                assert_eq!(codes, case.issues, "{}", case.name);
                if let Some(limits) = &case.limits {
                    let have: Vec<_> = have.iter().map(|i| i.limit.clone()).collect();
                    assert_eq!(&have, limits, "{}", case.name);
                }
            }
            (want, have) => panic!("{}: expected {want:?}, got {have:?}", case.name),
        }
    }
}

fn text_rule() -> FieldRule {
    FieldRule {
        kind: FieldKind::Text,
        nullable: false,
        unit: None,
        min: None,
        max: None,
        min_length: None,
        max_length: None,
        pattern: None,
        allowed: None,
        empty_is_null: false,
    }
}

#[test]
fn issue_order_is_declaration_order() {
    let mut rule = text_rule();
    rule.min_length = Some(9);
    rule.max_length = Some(1);
    rule.pattern = Some("x".into());
    rule.allowed = Some(vec![]);
    let issues = validate(&rule, &DbValue::Text("abc".into())).unwrap_err();
    let codes: Vec<_> = issues.iter().map(|i| i.code).collect();
    assert_eq!(
        codes,
        [
            FieldIssueCode::TooShort,
            FieldIssueCode::TooLong,
            FieldIssueCode::PatternMismatch,
            FieldIssueCode::NotAllowed
        ]
    );
    assert!(codes.is_sorted());
    assert_eq!(
        serde_json::to_string(&FieldIssueCode::BelowMin).unwrap(),
        "\"belowMin\""
    );
}

#[test]
fn pattern_is_anchored() {
    let mut rule = text_rule();
    rule.pattern = Some("a".into());
    assert!(validate(&rule, &DbValue::Text("a".into())).is_ok());
    assert!(validate(&rule, &DbValue::Text("ab".into())).is_err());
    rule.pattern = Some("a|b".into());
    assert!(validate(&rule, &DbValue::Text("ab".into())).is_err());
}

const UNITS: [DurationUnit; 5] = [
    DurationUnit::Milliseconds,
    DurationUnit::Seconds,
    DurationUnit::Minutes,
    DurationUnit::Hours,
    DurationUnit::Days,
];

#[test]
fn duration_parse_format_round_trip() {
    for unit in UNITS {
        let mut counts = vec![0, 1, -1, 59, 60, 61, 1440, -1500, 86_399, 1_000_003];
        counts.extend([i64::MAX, i64::MIN, i64::MAX - 1, i64::MIN + 1]);
        for count in counts {
            let text = duration::format(count, unit);
            assert_eq!(duration::parse(&text, unit), Ok(count), "{text} {unit:?}");
            // No component below the unit's own suffix.
            let forbidden: &[&str] = match unit {
                DurationUnit::Milliseconds => &[],
                DurationUnit::Seconds => &["ms"],
                DurationUnit::Minutes => &["ms", "s"],
                DurationUnit::Hours => &["ms", "s", "m"],
                DurationUnit::Days => &["ms", "s", "m", "h"],
            };
            let units: Vec<&str> = text
                .split(|c: char| c.is_ascii_digit() || c == '-')
                .filter(|p| !p.is_empty())
                .collect();
            assert!(units.iter().all(|u| !forbidden.contains(u)), "{text}");
        }
    }
    // i64::MIN days or hours overflows nothing in i128, but parse of the
    // text must still fit i64.
    assert_eq!(
        duration::parse("-9223372036854775808", DurationUnit::Days),
        Ok(i64::MIN)
    );
}

#[test]
fn duration_canonical_examples() {
    use DurationUnit::{Days, Milliseconds, Minutes, Seconds};
    assert_eq!(duration::format(5400, Seconds), "1h30m");
    assert_eq!(duration::format(1500, Milliseconds), "1s500ms");
    assert_eq!(duration::format(0, Minutes), "0m");
    assert_eq!(duration::format(-90, Seconds), "-1m30s");
    assert_eq!(duration::format(3, Days), "3d");
    assert_eq!(duration::format(90, Minutes), "1h30m");
    assert_eq!(duration::format(61_000, Milliseconds), "1m1s");
}

#[test]
fn duration_grammar_rejects() {
    let s = DurationUnit::Seconds;
    for bad in [
        "", "-", " 1h", "1h ", "1h  30m", "30m1h", "1h1h", "1H", "1.5h", "1h30", "1 h", "--1s",
        "+1s", "5w", "s", "1s ", "1h 30m ",
    ] {
        assert_eq!(
            duration::parse(bad, s),
            Err(DurationError::Grammar),
            "{bad:?}"
        );
    }
    assert_eq!(
        duration::parse("9223372036854775808", s),
        Err(DurationError::Overflow)
    );
    assert_eq!(
        duration::parse("200000000000d", DurationUnit::Milliseconds),
        Err(DurationError::Overflow)
    );
    assert_eq!(duration::parse("1500ms", s), Err(DurationError::Precision));
}
