//! FLAGS.md lists every `// FLAG:` in the code, and nothing else.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn rule_id(text: &str) -> Option<String> {
    let id: String = text
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    (id.starts_with('R') && id.len() > 1).then_some(id)
}

#[test]
fn flags_md_matches_the_flag_comments() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut in_code = BTreeSet::new();
    for entry in fs::read_dir(root.join("src")).unwrap() {
        let source =
            fs::read_to_string(entry.unwrap().path()).unwrap();
        for rest in source
            .lines()
            .filter_map(|l| l.trim_start().strip_prefix("// FLAG:"))
        {
            in_code.insert(
                rule_id(rest)
                    .expect("// FLAG: starts with a rule ID"),
            );
        }
    }
    let flags_md = fs::read_to_string(root.join("FLAGS.md")).unwrap();
    let listed: BTreeSet<String> = flags_md
        .lines()
        .filter_map(|l| l.strip_prefix("- **"))
        .filter_map(rule_id)
        .collect();
    assert_eq!(listed, in_code);
}
