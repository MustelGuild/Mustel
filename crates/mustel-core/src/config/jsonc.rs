//! Leitura de JSONC (JSON com comentários).
//!
//! O FurLab removia comentários com a regex `//.*?$|/\*[\s\S]*?\*/`, que também apagava `//`
//! **dentro de strings** (ex.: um host `http://db` viraria `http:`). Aqui o stripper é consciente de
//! strings — divergência aceita e registrada em `oracle/known-divergences.md`.

/// Remove comentários de linha (`//`) e de bloco (`/* */`) preservando o conteúdo das strings.
pub fn strip_comments(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut output = String::with_capacity(input.len());
    let mut index = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    while index < chars.len() {
        let current = chars[index];

        if in_string {
            output.push(current);
            if escaped {
                escaped = false;
            } else if current == '\\' {
                escaped = true;
            } else if current == '"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        match current {
            '"' => {
                in_string = true;
                output.push(current);
                index += 1;
            }
            '/' if chars.get(index + 1) == Some(&'/') => {
                while index < chars.len() && chars[index] != '\n' {
                    index += 1;
                }
            }
            '/' if chars.get(index + 1) == Some(&'*') => {
                index += 2;
                while index + 1 < chars.len() && !(chars[index] == '*' && chars[index + 1] == '/') {
                    index += 1;
                }
                index = (index + 2).min(chars.len());
            }
            _ => {
                output.push(current);
                index += 1;
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_line_and_block_comments() {
        let input = "{\n  // linha\n  \"a\": 1, /* bloco */\n  \"b\": 2\n}";
        let stripped = strip_comments(input);
        assert!(!stripped.contains("linha"));
        assert!(!stripped.contains("bloco"));
        let value: serde_json::Value = serde_json::from_str(&stripped).expect("valid json");
        assert_eq!(value["a"], 1);
        assert_eq!(value["b"], 2);
    }

    #[test]
    fn keeps_double_slash_inside_strings() {
        let input = r#"{ "host": "http://db.local", "path": "a/*b*/c" }"#;
        let stripped = strip_comments(input);
        let value: serde_json::Value = serde_json::from_str(&stripped).expect("valid json");
        assert_eq!(value["host"], "http://db.local");
        assert_eq!(value["path"], "a/*b*/c");
    }

    #[test]
    fn handles_escaped_quotes() {
        let input = r#"{ "name": "say \"hi\" // not a comment" }"#;
        let stripped = strip_comments(input);
        let value: serde_json::Value = serde_json::from_str(&stripped).expect("valid json");
        assert_eq!(value["name"], "say \"hi\" // not a comment");
    }

    #[test]
    fn unterminated_block_comment_consumes_to_the_end() {
        let input = "{ \"a\": 1 } /* sem fim";
        let stripped = strip_comments(input);
        assert_eq!(stripped.trim(), "{ \"a\": 1 }");
    }
}
