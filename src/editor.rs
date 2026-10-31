use std::path::Path;
pub fn is_ip(path: &Path) -> bool {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
        .starts_with("ipset")
}
pub fn normalize(input: &str, ip: bool) -> String {
    let mut lines = Vec::new();
    for source in input.trim_start_matches('\u{feff}').lines() {
        let line = source.trim_start();
        if line.starts_with('#') {
            lines.push(line.to_owned());
            continue;
        }
        let mut token = String::new();
        for c in line.chars() {
            if c.is_whitespace() {
                if !token.is_empty() {
                    lines.push(std::mem::take(&mut token));
                }
            } else if if ip {
                c.is_ascii_hexdigit() || matches!(c, '.' | ':' | '/' | '-')
            } else {
                c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '*')
            } {
                token.push(c);
            }
        }
        if !token.is_empty() {
            lines.push(token);
        }
    }
    if !ip {
        for line in &mut lines {
            if line.len() > 4
                && line
                    .get(..4)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("www."))
            {
                line.drain(..4);
            }
        }
        let mut seen = std::collections::HashSet::new();
        lines.retain(|line| line.starts_with('#') || seen.insert(line.to_ascii_lowercase()));
    }
    lines.join("\n")
}
pub fn editable(input: &str, ip: bool) -> String {
    let mut s = normalize(input, ip);
    if !s.is_empty() {
        s.push('\n');
    }
    s
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn domains_and_comments() {
        assert_eq!(
            normalize(
                "one.org bad@host! *.site\r\n\n# Любой текст: @! / пробелы",
                false
            ),
            "one.org\nbadhost\n*.site\n# Любой текст: @! / пробелы"
        );
    }
    #[test]
    fn duplicate_domains_preserve_first_and_comments() {
        assert_eq!(
            normalize("steamgames.com\nsteamgames.com \nSTEAMGAMES.COM\n# comment\nother.org steamgames.com\n# comment", false),
            "steamgames.com\n# comment\nother.org\n# comment"
        );
    }
    #[test]
    fn www_prefix_is_removed_before_deduplication() {
        assert_eq!(
            normalize("www.example.com\nexample.com WWW.example.com\nstore.example.com\n*.www.example.com\n# www.example.com", false),
            "example.com\nstore.example.com\n*.www.example.com\n# www.example.com"
        );
    }
    #[test]
    fn subnets_and_ipv6() {
        assert_eq!(
            normalize("203.0.113.113/32 2001:db8::/32\r\n# подсеть", true),
            "203.0.113.113/32\n2001:db8::/32\n# подсеть"
        );
    }
    #[test]
    fn trailing_line() {
        for ip in [false, true] {
            assert_eq!(editable("one\n\n", ip), if ip { "e\n" } else { "one\n" });
            assert_eq!(editable("", ip), "");
        }
    }
}
