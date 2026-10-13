/// `kebab-case` to `camelCase` as v12's `patterns` template does it
/// (`/-([a-z])/g` to the letter in upper case): only a hyphen before an
/// ASCII lowercase letter goes, so `a-1` and `a-B` keep theirs.
pub(super) fn kebab_to_camel_case(name: &str) -> String {
    let mut camel = String::with_capacity(name.len());
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        match chars.peek() {
            Some(&next) if c == '-' && next.is_ascii_lowercase() => {
                camel.push(next.to_ascii_uppercase());
                chars.next();
            }
            _ => camel.push(c),
        }
    }
    camel
}

#[cfg(test)]
mod tests {
    use super::kebab_to_camel_case;

    #[test]
    fn converts_as_v12s_regex_replace_does() {
        // Each expected value is Node 24's
        // `name.replace(/-([a-z])/g, (_, c) => c.toUpperCase())`.
        let cases = [
            ("my-long-name", "myLongName"),
            ("name", "name"),
            ("a--b", "a-B"),
            ("a-1", "a-1"),
            ("a-B", "a-B"),
            ("-a", "A"),
            ("a-", "a-"),
            ("a-é", "a-é"),
            ("a-b-c", "aBC"),
        ];
        for (kebab, camel) in cases {
            assert_eq!(kebab_to_camel_case(kebab), camel, "{kebab}");
        }
    }
}
