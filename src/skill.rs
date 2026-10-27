// skill: embedded agent documentation (`hvolt skill list` / `hvolt skill show`).

/// Docs embedded into the binary at build time, published for the community audience.
///
/// Sources live in `docs/published/` — sanitised copies of the repo's internal agent
/// notes (no private host names, IPs, user names or device identifiers). They ship with
/// the binary so the same version of the tool always carries the matching documentation.
const HVOLT_SKILL: &str = include_str!("../docs/published/hvolt-skill.md");
const CLEAN_CODE: &str = include_str!("../docs/published/clean-code.md");
const HARMONY_DEVICE_FLOW: &str = include_str!("../docs/published/harmony-device-flow.md");

/// Skill id → embedded document.
pub fn find(name: &str) -> Option<(&'static str, &'static str)> {
    match name {
        "hvolt-skill" => Some(("hvolt-skill", HVOLT_SKILL)),
        "clean-code" => Some(("clean-code", CLEAN_CODE)),
        "harmony-device-flow" => Some(("harmony-device-flow", HARMONY_DEVICE_FLOW)),
        _ => None,
    }
}

/// All embedded skill ids, in display order (also the order used by the available hint).
pub fn all() -> &'static [(&'static str, &'static str)] {
    &[
        (
            "hvolt-skill",
            "hvolt 用法、架构与开发指南（社区发布版）[中文]",
        ),
        (
            "clean-code",
            "代码规范与验收闸（fmt/clippy/test 三道）[中文]",
        ),
        (
            "harmony-device-flow",
            "跨主机构建/签名/安装指南（含签名链与实测坑）[English]",
        ),
    ]
}

/// Embedded skill ids, in display order (for the unknown-id hint).
pub fn ids() -> Vec<String> {
    all().iter().map(|(id, _)| id.to_string()).collect()
}

/// Content hash of every embedded document. Two binaries with the same hvolt version
/// can still carry different docs; the hash tells them apart (the version stamp cannot).
pub fn docs_hash() -> String {
    hash_docs(all())
}

/// Hash of a (id, doc) slice — pure, so tests can verify sensitivity to content.
fn hash_docs(docs: &[(&str, &str)]) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (id, doc) in docs {
        id.hash(&mut h);
        doc.hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

/// Human-friendly listing (header + aligned description column).
pub fn print_list() {
    println!(
        "hvolt {} — embedded docs (content {})",
        env!("CARGO_PKG_VERSION"),
        docs_hash()
    );
    println!("{:<20} description", "id");
    for (id, desc) in all() {
        println!("{:<20} {}", id, desc);
    }
}

/// Machine-readable listing: one id per line, nothing else. Agents can consume this
/// without filtering the header rows.
pub fn print_list_plain() {
    for (id, _) in all() {
        println!("{}", id);
    }
}

/// Machine-readable listing: JSON array of {id, description}.
pub fn print_list_json() {
    let items: Vec<String> = all()
        .iter()
        .map(|(id, desc)| {
            format!(
                "    {{\"id\": \"{}\", \"description\": \"{}\"}}",
                id,
                desc.replace('"', "\\\"")
            )
        })
        .collect();
    println!("{{");
    println!("  \"version\": \"{}\",", env!("CARGO_PKG_VERSION"));
    println!("  \"content_hash\": \"{}\",", docs_hash());
    println!("  \"skills\": [");
    println!("{}", items.join(",\n"));
    println!("  ]");
    println!("}}");
}

/// Print one embedded skill. Returns `false` when the id is unknown.
pub fn print_skill(name: &str) -> bool {
    match find(name) {
        Some((_, doc)) => {
            println!("{}", doc);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod embedded_docs_tests {
    use super::{all, docs_hash, find, hash_docs, ids};

    /// Every registered skill must be resolvable by its own id (the list and the show
    /// command can never drift apart).
    #[test]
    fn every_listed_skill_is_resolvable() {
        for (id, _) in all() {
            assert!(find(id).is_some(), "listed skill {} not findable", id);
        }
    }

    /// The embedded docs are the community-published set: no private host names, user
    /// names, IP addresses, device identifiers or internal paths. A leak here ships in
    /// every binary and release.
    #[test]
    fn embedded_docs_contain_no_private_identifiers() {
        let private_patterns = [
            "hongyuncong",
            "xyzsu",
            "192.168.",
            "100.64.",
            "PLR-AL00",
            "DevEcoStudioProjects",
            "/Users/",
            "/home/",
            "tailnet",
            "better-harmony-app-build",
        ];
        for (id, doc) in all() {
            for pat in private_patterns {
                assert!(
                    !doc.contains(pat),
                    "skill {} leaks private pattern {:?}",
                    id,
                    pat
                );
            }
        }
    }

    /// The community set is exactly the three published docs; internal agent conventions
    /// (issue tracker, triage labels, domain layout) are not user-facing knowledge.
    #[test]
    fn community_set_is_exactly_three_docs() {
        let ids: Vec<&str> = all().iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            vec!["hvolt-skill", "clean-code", "harmony-device-flow"]
        );
    }

    /// Unknown ids resolve to nothing (the CLI then exits non-zero).
    #[test]
    fn unknown_skill_is_not_found() {
        assert!(find("domain").is_none());
        assert!(find("issue-tracker").is_none());
        assert!(find("triage-labels").is_none());
    }

    /// The content hash is stable within a build and distinguishes the same version with
    /// different documents (the version stamp alone cannot).
    #[test]
    fn content_hash_is_stable_and_distinct() {
        let h1 = docs_hash();
        let h2 = docs_hash();
        assert_eq!(h1, h2, "same content must hash identically");
        assert_eq!(h1.len(), 16, "expected 16 hex chars, got {}", h1);
        assert!(h1.chars().all(|c| c.is_ascii_hexdigit()), "{}", h1);

        // Different content (same doc set, changed text) must produce a different hash.
        let docs: Vec<(&str, &str)> = all().iter().map(|(id, d)| (*id, *d)).collect();
        let mut changed = docs.clone();
        let extra = format!("{}extra", changed[0].1);
        changed[0].1 = &extra;
        assert_ne!(hash_docs(&docs), hash_docs(&changed));
    }

    /// The available-hint order follows the display order (list and hint never disagree
    /// about the canonical ordering).
    #[test]
    fn ids_match_display_order() {
        let want: Vec<String> = all().iter().map(|(id, _)| id.to_string()).collect();
        assert_eq!(ids(), want);
    }
}
