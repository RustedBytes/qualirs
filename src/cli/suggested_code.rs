use crate::domain::smell::Smell;

pub(crate) fn suggested_code(smell: &Smell) -> Option<String> {
    let line = source_line(smell)?;
    let suggestion = suggested_line_for(smell, &line)?;

    (suggestion.trim() != line.trim()).then_some(suggestion)
}

fn source_line(smell: &Smell) -> Option<String> {
    let source = std::fs::read_to_string(&smell.location.file).ok()?;
    source
        .lines()
        .nth(smell.location.line_start.checked_sub(1)?)
        .map(str::to_string)
}

fn suggested_line_for(smell: &Smell, line: &str) -> Option<String> {
    if smell.confidence != crate::domain::smell::FindingConfidence::High {
        return None;
    }
    match smell.code.as_str() {
        // Formatting and iterator rewrites require receiver/trait and borrow
        // information that these line-only suggestions cannot establish.
        "Q0054" => None,
        "Q0058" | "Q0064" => remove_clone_call(line),
        "Q0059" => None,
        "Q0060" => rewrite_chars_count(line),
        "Q0076" => elide_lifetime(line),
        _ => None,
    }
}

fn remove_clone_call(line: &str) -> Option<String> {
    use syn::visit::Visit;
    struct Clones(usize);
    impl<'a> Visit<'a> for Clones {
        fn visit_expr_method_call(&mut self, c: &'a syn::ExprMethodCall) {
            if c.method == "clone" && c.args.is_empty() {
                self.0 += 1;
            }
            syn::visit::visit_expr_method_call(self, c);
        }
    }
    let block = syn::parse_str::<syn::Block>(&format!("{{\n{line}\n}}")).ok()?;
    let mut clones = Clones(0);
    clones.visit_block(&block);
    (clones.0 == 1 && line.matches(".clone()").count() == 1)
        .then(|| line.replacen(".clone()", "", 1))
}

fn rewrite_chars_count(line: &str) -> Option<String> {
    use syn::visit::Visit;
    struct Counts(usize);
    impl<'a> Visit<'a> for Counts {
        fn visit_expr_method_call(&mut self, c: &'a syn::ExprMethodCall) {
            if c.method == "count"
                && c.args.is_empty()
                && matches!(&*c.receiver, syn::Expr::MethodCall(m) if m.method == "chars" && m.args.is_empty())
            {
                self.0 += 1;
            }
            syn::visit::visit_expr_method_call(self, c);
        }
    }
    let block = syn::parse_str::<syn::Block>(&format!("{{\n{line}\n}}")).ok()?;
    let mut counts = Counts(0);
    counts.visit_block(&block);
    if counts.0 != 1 || line.matches(".chars().count()").count() != 1 {
        return None;
    }
    rewrite_chars_count_empty_check(line)
}

fn rewrite_chars_count_empty_check(line: &str) -> Option<String> {
    let pattern = ".chars().count()";
    let count_start = line.find(pattern)?;
    let receiver_start = receiver_start(line, count_start)?;
    let receiver = line.get(receiver_start..count_start)?.trim();
    if receiver.is_empty() {
        return None;
    }

    let rest = line.get(count_start + pattern.len()..)?;
    let (negated, trailing) = empty_check_tail(rest)?;
    let replacement = if negated {
        format!("!{receiver}.is_empty()")
    } else {
        format!("{receiver}.is_empty()")
    };

    Some(format!(
        "{}{}{}",
        &line[..receiver_start],
        replacement,
        trailing
    ))
}

fn empty_check_tail(rest: &str) -> Option<(bool, &str)> {
    let rest = rest.trim_start();
    for (operator, negated) in [("==", false), ("!=", true), (">", true)] {
        let Some(after_operator) = rest.strip_prefix(operator).map(str::trim_start) else {
            continue;
        };
        let Some(after_zero) = after_operator.strip_prefix('0') else {
            continue;
        };
        if after_zero
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.'))
        {
            return None;
        }
        return Some((negated, after_zero));
    }

    None
}

fn elide_lifetime(line: &str) -> Option<String> {
    let parsed = syn::parse_str::<syn::ItemFn>(line).ok().or_else(|| {
        let signature = line.trim().strip_suffix('{')?.trim_end();
        syn::parse_str::<syn::ItemFn>(&format!("{signature} {{}}")).ok()
    })?;
    if !crate::detectors::implementation::needless_explicit_lifetime::elidable(&parsed) {
        return None;
    }
    let fn_start = line.find("fn ")?;
    let generics_start = line[fn_start..].find('<')? + fn_start;
    let generics_end = line[generics_start..].find('>')? + generics_start;
    let paren_start = line[fn_start..].find('(')? + fn_start;
    if generics_start > paren_start {
        return None;
    }

    let lifetime = line[generics_start + 1..generics_end].trim();
    if !is_single_lifetime_param(lifetime) {
        return None;
    }

    let mut rewritten = format!("{}{}", &line[..generics_start], &line[generics_end + 1..]);
    rewritten = rewritten.replace(&format!("&{lifetime} mut "), "&mut ");
    rewritten = rewritten.replace(&format!("&{lifetime} "), "&");
    Some(rewritten)
}

fn is_single_lifetime_param(value: &str) -> bool {
    let Some(name) = value.strip_prefix('\'') else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn receiver_start(line: &str, receiver_end: usize) -> Option<usize> {
    let prefix = line.get(..receiver_end)?;
    let start = prefix
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_receiver_char(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);

    (start < receiver_end).then_some(start)
}

fn is_receiver_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '.')
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};

    use super::*;

    #[test]
    fn removes_clone_from_copy_values() {
        let smell = smell("Clone on Copy");

        assert_eq!(
            suggested_line_for(&smell, "    count.clone()").as_deref(),
            Some("    count")
        );
    }

    #[test]
    fn omits_iterator_rewrite_without_type_evidence() {
        let smell = smell("Inefficient Iterator Step");

        assert_eq!(
            suggested_line_for(&smell, "    values.skip(3).next()").as_deref(),
            None
        );
    }

    #[test]
    fn rewrites_chars_count_zero_check_to_is_empty() {
        let smell = smell("Chars Count Length Check");

        assert_eq!(
            suggested_line_for(&smell, "    value.chars().count() == 0").as_deref(),
            Some("    value.is_empty()")
        );
    }

    #[test]
    fn omits_formatting_rewrite_without_borrow_evidence() {
        let smell = smell("Needless Intermediate String Formatting");

        assert_eq!(
            suggested_line_for(&smell, r#"    line.push_str(&format!("id={id}"));"#).as_deref(),
            None
        );
    }

    #[test]
    fn elides_simple_named_lifetime() {
        let smell = smell("Needless Explicit Lifetime");

        assert_eq!(
            suggested_line_for(
                &smell,
                "fn needless_lifetime<'a>(value: &'a str) -> &'a str {"
            )
            .as_deref(),
            Some("fn needless_lifetime(value: &str) -> &str {")
        );
    }

    #[test]
    fn omits_ambiguous_or_semantically_different_rewrites() {
        let chars = smell("Chars Count Length Check");
        for line in [
            "s.chars().count() == 2",
            "s.chars().count() + 1",
            "s.chars().count() == 0usize",
            "(a.chars().count() == 0, b.chars().count() == 0)",
            "\"s.chars().count() == 0\"",
        ] {
            assert_eq!(suggested_line_for(&chars, line), None, "{line}");
        }
        assert_eq!(
            suggested_line_for(&smell("Clone on Copy"), "(a.clone(), b.clone())"),
            None
        );
        assert_eq!(
            suggested_line_for(
                &smell("Needless Explicit Lifetime"),
                "fn f<'a: 'static>(x: &'a str) -> &'static str { x }"
            ),
            None
        );
        let mut uncertain = chars;
        uncertain.confidence = crate::domain::smell::FindingConfidence::Low;
        assert_eq!(
            suggested_line_for(&uncertain, "s.chars().count() == 0"),
            None
        );
    }

    #[test]
    fn generated_replacements_compile_and_preserve_results() {
        let empty =
            suggested_line_for(&smell("Chars Count Length Check"), "s.chars().count() == 0")
                .unwrap();
        let nonempty =
            suggested_line_for(&smell("Chars Count Length Check"), "s.chars().count() > 0")
                .unwrap();
        let copy = suggested_line_for(&smell("Clone on Copy"), "value.clone()").unwrap();
        let lifetime = suggested_line_for(
            &smell("Needless Explicit Lifetime"),
            "fn identity<'a>(s: &'a str) -> &'a str { s }",
        )
        .unwrap();
        let source = format!(
            r#"
fn empty(s: &str) -> bool {{ {empty} }}
fn nonempty(s: &str) -> bool {{ {nonempty} }}
fn copy(value: u32) -> u32 {{ {copy} }}
{lifetime}
fn main() {{
    for s in ["", "hello", "é", "😀", "e\u{{301}}"] {{
        assert_eq!(empty(s), s.chars().count() == 0);
        assert_eq!(nonempty(s), s.chars().count() > 0);
        assert_eq!(identity(s), s);
    }}
    for value in [0, 1, u32::MAX] {{ assert_eq!(copy(value), value.clone()); }}
}}
"#
        );
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("suggestions.rs");
        let output = dir
            .path()
            .join(format!("suggestions{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&input, source).unwrap();
        let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let result = std::process::Command::new(compiler)
            .arg("--edition=2024")
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            std::process::Command::new(output)
                .status()
                .unwrap()
                .success()
        );
    }

    fn smell(name: &str) -> Smell {
        Smell::new(
            SmellCategory::Implementation,
            name,
            Severity::Info,
            crate::domain::smell::FindingConfidence::High,
            SourceLocation::new(PathBuf::from("src/lib.rs"), 1, 1, None),
            "message",
            "suggestion",
        )
    }
}
