//! Source scans for rules a missed code review would break. Each test names
//! the rule and the file that broke it.

use std::{
    fs,
    path::{Path, PathBuf},
};

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// (path relative to `crates/`, contents) for every `.rs` file under `crate_name/src`.
fn sources(crate_name: &str) -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let root = crates_dir();
    let mut files = Vec::new();
    walk(&root.join(crate_name).join("src"), &mut files);
    assert!(!files.is_empty(), "no sources found for {crate_name}");
    files
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (rel, fs::read_to_string(&p).unwrap())
        })
        .collect()
}

/// Lines (1-based) containing `needle`, ignoring `//` comment lines.
fn hits(src: &str, needle: &str) -> Vec<usize> {
    src.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim_start().starts_with("//") && l.contains(needle))
        .map(|(i, _)| i + 1)
        .collect()
}

fn assert_absent(crate_names: &[&str], needles: &[&str], allowed_files: &[&str], why: &str) {
    let mut found = Vec::new();
    for name in crate_names {
        for (file, src) in sources(name) {
            if allowed_files.contains(&file.as_str()) {
                continue;
            }
            for needle in needles {
                for line in hits(&src, needle) {
                    found.push(format!("{file}:{line} uses `{needle}`"));
                }
            }
        }
    }
    assert!(found.is_empty(), "{why}\n{}", found.join("\n"));
}

#[test]
fn inner_layers_name_no_framework_or_driver() {
    assert_absent(
        &["domain", "application"],
        &[
            "sea_orm::",
            "jsonwebtoken::",
            "bcrypt::",
            "lettre::",
            "tower::",
            "tower_http::",
            "tera::",
        ],
        &[],
        "domain/application must reach infrastructure only through port and repository traits",
    );
    // axum is allowed in exactly one place: the feature-gated error impls.
    assert_absent(
        &["domain", "application"],
        &["axum::"],
        &["domain/src/error.rs"],
        "axum outside domain's feature-gated error module",
    );
}

#[test]
fn only_infrastructure_names_drivers() {
    assert_absent(
        &["web"],
        &["jsonwebtoken::", "bcrypt::", "lettre::"],
        &[],
        "the web layer must use the TokenService/PasswordHasher/EmailService ports",
    );
}

#[test]
fn environment_is_read_only_in_startup() {
    assert_absent(
        &["domain", "application", "infrastructure", "web"],
        &["std::env::var", "env::var("],
        &["web/src/startup.rs"],
        "configuration must be read in startup.rs and passed in, so tests can supply it",
    );
}

#[test]
fn handlers_use_the_coded_extractors() {
    // axum's own extractors reject with plain-text bodies that bypass the
    // localised error renderer.
    assert_absent(
        &["web"],
        &["Json(req): Json<", ": Path<", ": Query<", "Option<Json<"],
        &[],
        "use ApiJson/ApiPath/ApiQuery from web::extract",
    );
}

#[test]
fn handlers_build_no_ad_hoc_error_responses() {
    assert_absent(
        &["web"],
        &[
            "StatusCode::BAD_REQUEST",
            "StatusCode::UNAUTHORIZED",
            "StatusCode::FORBIDDEN",
            "StatusCode::NOT_FOUND",
            "StatusCode::CONFLICT",
            "StatusCode::INTERNAL_SERVER_ERROR",
            "StatusCode::TOO_MANY_REQUESTS",
            "json!(",
        ],
        &[],
        "return a domain::Error with a code; the renderer picks status and text",
    );
}

#[test]
fn time_is_never_read_in_the_local_zone() {
    assert_absent(
        &["domain", "application", "infrastructure", "web"],
        &["Local::now", "chrono::Local"],
        &[],
        "instants are DateTime<Utc>; the server's zone is not the user's",
    );
}

#[test]
fn every_lint_free_crate_opts_into_workspace_lints() {
    for name in ["domain", "application", "infrastructure", "web"] {
        let manifest = fs::read_to_string(crates_dir().join(name).join("Cargo.toml")).unwrap();
        assert!(
            manifest.contains("[lints]\nworkspace = true"),
            "{name}/Cargo.toml must opt into [workspace.lints]"
        );
    }
    let migration = fs::read_to_string(crates_dir().join("../migration/Cargo.toml")).unwrap();
    assert!(
        migration.contains("[lints]\nworkspace = true"),
        "migration/Cargo.toml must opt into [workspace.lints]"
    );
}

#[test]
fn third_party_versions_live_in_the_workspace() {
    // A version pinned in a member drifts from the rest of the workspace.
    for name in ["domain", "application", "infrastructure", "web", "tests"] {
        let manifest = fs::read_to_string(crates_dir().join(name).join("Cargo.toml")).unwrap();
        for line in manifest.lines() {
            let line = line.trim();
            let is_dep_line =
                line.contains('=') && !line.starts_with('#') && !line.starts_with('[');
            if is_dep_line && line.contains("version") && !line.contains("workspace") {
                panic!("{name}/Cargo.toml pins a version outside [workspace.dependencies]: {line}");
            }
        }
    }
}

// ── Templates ────────────────────────────────────────────────────────────────

fn templates() -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let root = crates_dir().join("web").join("templates");
    let mut files = Vec::new();
    walk(&root, &mut files);
    assert!(!files.is_empty(), "no templates found");
    files
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (rel, fs::read_to_string(&p).unwrap())
        })
        .collect()
}

fn template_violations(pattern: &str, why: &str) {
    let re = regex::Regex::new(pattern).unwrap();
    let found: Vec<String> = templates()
        .iter()
        .flat_map(|(file, src)| {
            src.lines()
                .enumerate()
                .filter(|(_, l)| re.is_match(l))
                .map(move |(i, l)| format!("{file}:{}: {}", i + 1, l.trim()))
        })
        .collect();
    assert!(found.is_empty(), "{why}\n{}", found.join("\n"));
}

#[test]
fn templates_are_html_so_tera_escapes_them() {
    // Tera autoescapes by file extension; a .txt or .j2 template would render
    // user text unescaped.
    for (file, _) in templates() {
        assert!(
            file.ends_with(".html"),
            "{file}: templates must end in .html"
        );
    }
}

#[test]
fn templates_use_tera_comments() {
    template_violations("<!--", "HTML comments ship to the browser; use {# #}");
}

#[test]
fn templates_never_mark_output_safe() {
    template_violations(
        r"\|\s*safe\b",
        "`| safe` disables escaping; render HTML through a macro instead",
    );
}

#[test]
fn templates_have_no_inline_styles() {
    template_violations(
        r#"\sstyle\s*="#,
        "the page CSP is style-src 'self'; inline style attributes are blocked",
    );
}

#[test]
fn template_attributes_are_quoted() {
    // The escaper leaves `/`, `=` and spaces alone, which is only safe inside
    // a quoted attribute value.
    template_violations(
        r"=\s*\{\{",
        "quote every attribute: href=\"{{ x }}\", never href={{ x }}",
    );
}

#[test]
fn alpine_attributes_name_registered_members_only() {
    // The CSP build cannot evaluate expressions; `@click="open = !open"` would
    // fail silently in the browser.
    let attr = regex::Regex::new(r#"\s(x-[a-z:.-]+|@[a-z.:-]+|:[a-z-]+)="([^"]*)""#).unwrap();
    let ident = regex::Regex::new(r"^[A-Za-z_$][A-Za-z0-9_.$]*$").unwrap();
    let mut found = Vec::new();
    for (file, src) in templates() {
        for cap in attr.captures_iter(&src) {
            let (name, value) = (&cap[1], &cap[2]);
            if !ident.is_match(value) {
                found.push(format!("{file}: {name}=\"{value}\""));
            }
        }
    }
    assert!(
        found.is_empty(),
        "Alpine attributes must be plain identifiers:\n{}",
        found.join("\n")
    );
}

#[test]
fn only_views_rs_talks_to_tera() {
    assert_absent(
        &["web"],
        &["tera::", ".render("],
        &["web/src/views.rs"],
        "render through views::render_page so every page gets the shared context and headers",
    );
}
