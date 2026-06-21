#[derive(Clone, Copy, Default)]
pub(crate) enum RenameRuleAttr {
    #[default]
    None,
    SnakeCase,
    KebabCase,
    CamelCase,
    PascalCase,
}

impl RenameRuleAttr {
    const SNAKE_CASE: &'static str = "snake_case";
    const KEBAB_CASE: &'static str = "kebab-case";
    const CAMEL_CASE: &'static str = "camelCase";
    const CAMEL_CASE_ALIAS: &'static str = "camel_case";
    const PASCAL_CASE: &'static str = "PascalCase";
    const PASCAL_CASE_ALIAS: &'static str = "pascal_case";

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            Self::SNAKE_CASE => Some(Self::SnakeCase),
            Self::KEBAB_CASE => Some(Self::KebabCase),
            Self::CAMEL_CASE | Self::CAMEL_CASE_ALIAS => Some(Self::CamelCase),
            Self::PASCAL_CASE | Self::PASCAL_CASE_ALIAS => Some(Self::PascalCase),
            _ => None,
        }
    }

    pub(crate) fn apply(self, input: &str) -> String {
        match self {
            Self::None => input.to_owned(),
            Self::SnakeCase => to_words(input).join("_"),
            Self::KebabCase => to_words(input).join("-"),
            Self::CamelCase => {
                let mut words = to_words(input).into_iter();
                match words.next() {
                    Some(first) => words.fold(first, |mut out, word| {
                        push_pascal(&mut out, &word);
                        out
                    }),
                    None => String::new(),
                }
            }
            Self::PascalCase => to_words(input)
                .into_iter()
                .fold(String::new(), |mut out, word| {
                    push_pascal(&mut out, &word);
                    out
                }),
        }
    }
}

fn push_pascal(out: &mut String, word: &str) {
    let mut chars = word.chars();
    if let Some(first) = chars.next() {
        out.extend(first.to_uppercase());
        out.push_str(chars.as_str());
    }
}

fn to_words(input: &str) -> Vec<String> {
    let (mut words, current) = input.chars().fold(
        (Vec::<String>::new(), String::new()),
        |(mut words, mut current), ch| {
            if ch == '_' || ch == '-' {
                if !current.is_empty() {
                    words.push(current.clone());
                    current.clear();
                }
                return (words, current);
            }
            if ch.is_uppercase() && !current.is_empty() {
                words.push(current.clone());
                current.clear();
            }
            current.extend(ch.to_lowercase());
            (words, current)
        },
    );
    if !current.is_empty() {
        words.push(current);
    }
    words
}
