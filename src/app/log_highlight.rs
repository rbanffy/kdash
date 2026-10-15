use std::ops::Range;

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum JsonHighlight {
  Key,
  ErrorLevel,
  WarningLevel,
}

pub(super) fn json_log_highlights(line: &str) -> Vec<(Range<usize>, JsonHighlight)> {
  let Some(json_start) = find_json_object_start(line) else {
    return Vec::new();
  };

  let bytes = line.as_bytes();
  let mut highlights = Vec::new();
  let mut cursor = json_start;

  while cursor < bytes.len() {
    if bytes[cursor] != b'"' {
      cursor += 1;
      continue;
    }

    let Some(end) = json_string_end(bytes, cursor) else {
      break;
    };
    let token = &line[cursor..end];
    let key = serde_json::from_str::<String>(token).ok();
    let mut next = end;
    skip_json_whitespace(bytes, &mut next);

    if bytes.get(next) == Some(&b':') {
      if let Some(key) = key {
        highlights.push((cursor..end, JsonHighlight::Key));
        if key == "level" {
          next += 1;
          skip_json_whitespace(bytes, &mut next);
          if bytes.get(next) == Some(&b'"') {
            if let Some(value_end) = json_string_end(bytes, next) {
              if let Ok(value) = serde_json::from_str::<String>(&line[next..value_end]) {
                let highlight = if value.eq_ignore_ascii_case("error") {
                  Some(JsonHighlight::ErrorLevel)
                } else if value.eq_ignore_ascii_case("warn")
                  || value.eq_ignore_ascii_case("warning")
                {
                  Some(JsonHighlight::WarningLevel)
                } else {
                  None
                };
                if let Some(highlight) = highlight {
                  highlights.push((next..value_end, highlight));
                }
              }
            }
          }
        }
      }
    }

    cursor = end;
  }

  highlights
}

fn find_json_object_start(line: &str) -> Option<usize> {
  for (start, _) in line.match_indices('{') {
    let mut deserializer = serde_json::Deserializer::from_str(&line[start..]);
    if <serde::de::IgnoredAny>::deserialize(&mut deserializer).is_ok() && deserializer.end().is_ok()
    {
      return Some(start);
    }
  }
  None
}

fn json_string_end(bytes: &[u8], start: usize) -> Option<usize> {
  let mut cursor = start + 1;
  while cursor < bytes.len() {
    match bytes[cursor] {
      b'\\' => cursor = cursor.checked_add(2)?,
      b'"' => return Some(cursor + 1),
      _ => cursor += 1,
    }
  }
  None
}

fn skip_json_whitespace(bytes: &[u8], cursor: &mut usize) {
  while bytes.get(*cursor).is_some_and(u8::is_ascii_whitespace) {
    *cursor += 1;
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn highlighted_text(line: &str, highlight: JsonHighlight) -> Vec<&str> {
    json_log_highlights(line)
      .into_iter()
      .filter_map(|(range, found)| (found == highlight).then_some(&line[range]))
      .collect()
  }

  #[test]
  fn highlights_prefixed_json_logs() {
    let line = r#"2026-10-15T18:00:00Z [pod-name] {"level":"error","message":"bad"}"#;
    assert_eq!(
      highlighted_text(line, JsonHighlight::Key),
      vec![r#""level""#, r#""message""#]
    );
    assert_eq!(
      highlighted_text(line, JsonHighlight::ErrorLevel),
      vec![r#""error""#]
    );
  }

  #[test]
  fn highlights_warning_levels() {
    let line = r#"{"level":"warning","message":"slow"}"#;
    assert_eq!(
      highlighted_text(line, JsonHighlight::WarningLevel),
      vec![r#""warning""#]
    );
  }

  #[test]
  fn rejects_invalid_or_non_object_json_suffixes() {
    assert!(json_log_highlights(r#"prefix {"level":"error"} trailing"#).is_empty());
    assert!(json_log_highlights(r#"not json without an object"#).is_empty());
  }
}
