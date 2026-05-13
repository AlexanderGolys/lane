use lane::compile_program_from_path;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn compile_pass_fixtures_compile_without_errors() {
    for lane_path in fixture_files("tests/fixtures/compile-pass", "lane") {
        compile_program_from_path(&lane_path)
            .unwrap_or_else(|err| panic!("{} failed to compile: {err}", lane_path.display()));
    }
}

#[test]
fn glsl_compare_fixtures_match_expected_output() {
    for lane_path in fixture_files("tests/fixtures/glsl-compare", "lane") {
        let expected_path = lane_path.with_extension("glsl");
        let expected = fs::read_to_string(&expected_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", expected_path.display()));
        let actual = compile_program_from_path(&lane_path)
            .unwrap_or_else(|err| panic!("{} failed to compile: {err}", lane_path.display()));

        assert_glsl_equivalent(&actual, &expected, &lane_path);
    }
}

fn fixture_files(dir: impl AsRef<Path>, extension: &str) -> Vec<PathBuf> {
    let mut files = fs::read_dir(dir.as_ref())
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.as_ref().display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|actual| actual == extension))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn assert_glsl_equivalent(actual: &str, expected: &str, lane_path: &Path) {
    let actual = canonical_glsl(actual);
    let expected = canonical_glsl(expected);
    assert_eq!(
        actual,
        expected,
        "generated GLSL did not match {}",
        lane_path.display()
    );
}

fn canonical_glsl(source: &str) -> String {
    let normalized = normalize_single_underscore_identifiers(source);
    let mut declarations = split_top_level_declarations(&normalized)
        .into_iter()
        .map(|decl| collapse_whitespace(&decl))
        .filter(|decl| !decl.is_empty())
        .collect::<Vec<_>>();
    declarations.sort();
    declarations.join("\n")
}

fn normalize_single_underscore_identifiers(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '_'
            && chars.get(index + 1).is_some_and(|ch| is_ident_start(*ch))
            && (index == 0 || !is_ident_continue(chars[index - 1]))
        {
            let next = index + 1;
            if chars.get(next) != Some(&'_') {
                index += 1;
                continue;
            }
        }
        out.push(chars[index]);
        index += 1;
    }
    out
}

fn split_top_level_declarations(source: &str) -> Vec<String> {
    let mut declarations = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;

    for ch in source.chars() {
        current.push(ch);
        let previous_depth = depth;
        match ch {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => depth = depth.saturating_sub(1),
            ';' if depth == 0 => declarations.push(std::mem::take(&mut current)),
            _ => {}
        }
        if ch == '}' && previous_depth == 1 && depth == 0 {
            declarations.push(std::mem::take(&mut current));
        }
    }
    if !current.trim().is_empty() {
        declarations.push(current);
    }
    declarations
}

fn collapse_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
