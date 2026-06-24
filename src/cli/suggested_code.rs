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
    match smell.code.as_str() {
        "Q0054" => rewrite_push_str_format(line),
        "Q0058" | "Q0064" => remove_clone_call(line),
        "Q0059" => rewrite_iterator_step(line),
        "Q0060" => rewrite_chars_count(line),
        "Q0076" => elide_lifetime(line),
        _ => None,
    }
}

fn remove_clone_call(line: &str) -> Option<String> {
    line.contains(".clone()")
        .then(|| line.replacen(".clone()", "", 1))
}

fn rewrite_iterator_step(line: &str) -> Option<String> {
    if line.contains(".nth(0)") {
        return Some(line.replacen(".nth(0)", ".next()", 1));
    }

    rewrite_skip_next(line)
}

fn rewrite_skip_next(line: &str) -> Option<String> {
    let skip_start = line.find(".skip(")?;
    let open_paren = skip_start + ".skip".len();
    let close_paren = matching_paren(line, open_paren)?;
    let next_start = close_paren + 1;
    let next_call = ".next()";
    if !line.get(next_start..)?.starts_with(next_call) {
        return None;
    }

    let arg_start = open_paren + 1;
    let arg = line.get(arg_start..close_paren)?.trim();
    if arg.is_empty() {
        return None;
    }

    Some(format!(
        "{}.nth({}){}",
        &line[..skip_start],
        arg,
        &line[next_start + next_call.len()..]
    ))
}

fn rewrite_chars_count(line: &str) -> Option<String> {
    rewrite_chars_count_empty_check(line).or_else(|| {
        line.contains(".chars().count()")
            .then(|| line.replacen(".chars().count()", ".len()", 1))
    })
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
            .is_some_and(|ch| ch.is_ascii_digit())
        {
            return None;
        }
        return Some((negated, after_zero));
    }

    None
}

fn rewrite_push_str_format(line: &str) -> Option<String> {
    let method_start = line.find(".push_str(")?;
    let receiver_start = receiver_start(line, method_start)?;
    let receiver = line.get(receiver_start..method_start)?.trim();
    if receiver.is_empty() {
        return None;
    }

    let open_paren = method_start + ".push_str".len();
    let close_paren = matching_paren(line, open_paren)?;
    let argument = line.get(open_paren + 1..close_paren)?.trim();
    let macro_args = argument
        .strip_prefix("&format!(")?
        .strip_suffix(')')?
        .trim();
    if macro_args.is_empty() {
        return None;
    }

    Some(format!(
        "{}use std::fmt::Write as _;\n{}write!(&mut {}, {}).expect(\"write to String\");",
        &line[..receiver_start],
        &line[..receiver_start],
        receiver,
        macro_args
    ))
}

fn elide_lifetime(line: &str) -> Option<String> {
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

fn matching_paren(text: &str, open_paren: usize) -> Option<usize> {
    if text.as_bytes().get(open_paren) != Some(&b'(') {
        return None;
    }

    let mut depth = 0usize;
    for (offset, ch) in text[open_paren..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open_paren + offset);
                }
            }
            _ => {}
        }
    }

    None
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
    fn rewrites_skip_next_to_nth() {
        let smell = smell("Inefficient Iterator Step");

        assert_eq!(
            suggested_line_for(&smell, "    values.skip(3).next()").as_deref(),
            Some("    values.nth(3)")
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
    fn rewrites_push_str_format_to_write_macro() {
        let smell = smell("Needless Intermediate String Formatting");

        assert_eq!(
            suggested_line_for(&smell, r#"    line.push_str(&format!("id={id}"));"#).as_deref(),
            Some(
                "    use std::fmt::Write as _;\n    write!(&mut line, \"id={id}\").expect(\"write to String\");"
            )
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
