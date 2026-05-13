use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenCost {
    pub bytes: usize,
    pub chars: usize,
    pub lexemes: usize,
    pub llm_tokens: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetCost {
    pub name: String,
    pub cost: TokenCost,
}

pub fn token_cost(src: &str) -> TokenCost {
    let bytes = src.len();
    let chars = src.chars().count();
    let lexemes = lexeme_count(src);
    let llm_tokens = llmish_token_count(src);
    let total = bytes + chars + (lexemes * 4) + (llm_tokens * 8);
    TokenCost {
        bytes,
        chars,
        lexemes,
        llm_tokens,
        total,
    }
}

pub fn compare_snippets<const N: usize>(snippets: [(&str, &str); N]) -> Vec<SnippetCost> {
    let mut rows = snippets
        .into_iter()
        .map(|(name, src)| SnippetCost {
            name: name.to_string(),
            cost: token_cost(src),
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        a.cost
            .total
            .cmp(&b.cost.total)
            .then_with(|| a.name.cmp(&b.name))
    });
    rows
}

fn lexeme_count(src: &str) -> usize {
    let mut count = 0;
    let mut chars = src.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }

        count += 1;
        if ch.is_ascii_alphanumeric() || ch == '_' {
            while chars
                .peek()
                .is_some_and(|next| next.is_ascii_alphanumeric() || *next == '_')
            {
                chars.next();
            }
        } else if ch == '"' {
            let mut escaped = false;
            for next in chars.by_ref() {
                if escaped {
                    escaped = false;
                } else if next == '\\' {
                    escaped = true;
                } else if next == '"' {
                    break;
                }
            }
        } else if is_two_char_starter(ch)
            && chars.peek().is_some_and(|next| is_two_char_op(ch, *next))
        {
            chars.next();
        }
    }
    count
}

fn llmish_token_count(src: &str) -> usize {
    let mut count = 0;
    for word in src.split(|ch: char| ch.is_whitespace() || is_punct(ch)) {
        if word.is_empty() {
            continue;
        }
        count += word.len().div_ceil(4).max(1);
    }
    count + src.chars().filter(|ch| is_punct(*ch)).count()
}

fn is_punct(ch: char) -> bool {
    "(){}[],:;.!?~&|^+-*/%=<>#\\_\"'".contains(ch)
}

fn is_two_char_starter(ch: char) -> bool {
    ":<>=!+-*.".contains(ch)
}

fn is_two_char_op(first: char, second: char) -> bool {
    matches!(
        (first, second),
        (':', '=')
            | ('<', '=')
            | ('>', '=')
            | ('=', '=')
            | ('!', '=')
            | ('+', '%')
            | ('-', '%')
            | ('*', '%')
            | ('+', '|')
            | ('-', '|')
            | ('*', '|')
            | ('+', '?')
            | ('-', '?')
            | ('*', '?')
            | ('.', '.')
    )
}
