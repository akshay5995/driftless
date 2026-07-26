use std::path::Path;

pub(crate) const CONFIG_FILE: &str = ".driftlessignore";

#[derive(Debug, Default)]
pub(crate) struct Config {
    pub(crate) exclude: Vec<String>,
}

pub(crate) fn load_config(root: &Path) -> Config {
    let Ok(text) = std::fs::read_to_string(root.join(CONFIG_FILE)) else {
        return Config::default();
    };
    let exclude = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect();
    Config { exclude }
}

pub(crate) fn is_excluded(rel_path: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| glob_match(p, rel_path))
}

/// Minimal glob matcher over `/`-separated path segments: `*` matches
/// within a single segment, `**` matches zero or more whole segments.
fn glob_match(pattern: &str, path: &str) -> bool {
    let pat_segs: Vec<&str> = pattern.split('/').collect();
    let path_segs: Vec<&str> = path.split('/').collect();
    match_segments(&pat_segs, &path_segs)
}

fn match_segments(pat: &[&str], path: &[&str]) -> bool {
    match pat.first() {
        None => path.is_empty(),
        Some(&"**") => {
            match_segments(&pat[1..], path) || (!path.is_empty() && match_segments(pat, &path[1..]))
        }
        Some(p) => {
            !path.is_empty() && segment_match(p, path[0]) && match_segments(&pat[1..], &path[1..])
        }
    }
}

fn segment_match(pattern: &str, segment: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == segment;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut rest = segment;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            let Some(stripped) = rest.strip_prefix(part) else {
                return false;
            };
            rest = stripped;
        } else if i == parts.len() - 1 {
            return rest.ends_with(part);
        } else if let Some(pos) = rest.find(part) {
            rest = &rest[pos + part.len()..];
        } else {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_has_no_excludes() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = load_config(dir.path());
        assert!(config.exclude.is_empty());
    }

    #[test]
    fn config_ignores_blank_lines_and_comments() {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(
            dir.path().join(CONFIG_FILE),
            "# comment\n\nvendor/**\n  docs/legacy/*.md  \n",
        )
        .expect("write config");

        let config = load_config(dir.path());

        assert_eq!(config.exclude, vec!["vendor/**", "docs/legacy/*.md"]);
    }

    #[test]
    fn double_star_matches_whole_subtree() {
        assert!(is_excluded("vendor/pkg/readme.md", &["vendor/**".into()]));
        assert!(is_excluded("vendor/readme.md", &["vendor/**".into()]));
        assert!(!is_excluded("src/vendor.md", &["vendor/**".into()]));
    }

    #[test]
    fn single_star_matches_within_a_segment() {
        assert!(is_excluded(
            "docs/legacy/old.md",
            &["docs/legacy/*.md".into()]
        ));
        assert!(!is_excluded(
            "docs/legacy/sub/old.md",
            &["docs/legacy/*.md".into()]
        ));
    }

    #[test]
    fn leading_double_star_matches_any_depth() {
        assert!(is_excluded(
            "a/b/c/old.generated.md",
            &["**/*.generated.md".into()]
        ));
        assert!(is_excluded(
            "old.generated.md",
            &["**/*.generated.md".into()]
        ));
    }
}
