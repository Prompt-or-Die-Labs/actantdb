//! Unit tests for the in-tree `{{key}}` substitution engine.

use std::collections::HashMap;

use actant_templates::substitute;

fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn replaces_single_placeholder() {
    let v = vars(&[("project_name", "my-app")]);
    assert_eq!(substitute("# {{project_name}}", &v), "# my-app");
}

#[test]
fn replaces_multiple_placeholders() {
    let v = vars(&[
        ("project_name", "demo"),
        ("port", "8400"),
        ("studio_port", "8401"),
    ]);
    let input = "name={{project_name}} port={{port}} studio={{studio_port}}";
    let want = "name=demo port=8400 studio=8401";
    assert_eq!(substitute(input, &v), want);
}

#[test]
fn tolerates_whitespace_inside_braces() {
    let v = vars(&[("project_name", "my-app")]);
    assert_eq!(substitute("{{ project_name }}", &v), "my-app");
    assert_eq!(substitute("{{  project_name  }}", &v), "my-app");
}

#[test]
fn leaves_unknown_keys_untouched() {
    let v = vars(&[("project_name", "demo")]);
    assert_eq!(
        substitute("{{project_name}} and {{unknown}}", &v),
        "demo and {{unknown}}"
    );
}

#[test]
fn leaves_non_key_content_alone() {
    let v: HashMap<String, String> = HashMap::new();
    // Spaces + arithmetic — not a valid key — must round-trip.
    assert_eq!(substitute("{{ 1 + 2 }}", &v), "{{ 1 + 2 }}");
}

#[test]
fn passes_text_without_placeholders_unchanged() {
    let v = vars(&[("project_name", "demo")]);
    assert_eq!(
        substitute("plain text with { single braces } only", &v),
        "plain text with { single braces } only"
    );
}

#[test]
fn handles_consecutive_placeholders() {
    let v = vars(&[("a", "1"), ("b", "2")]);
    assert_eq!(substitute("{{a}}{{b}}", &v), "12");
}

#[test]
fn handles_multibyte_text() {
    let v = vars(&[("project_name", "demo")]);
    // emoji + non-ascii around the placeholder.
    let input = "héllo {{project_name}} 🚀";
    let want = "héllo demo 🚀";
    assert_eq!(substitute(input, &v), want);
}

#[test]
fn unterminated_brace_passes_through() {
    let v = vars(&[("x", "y")]);
    assert_eq!(substitute("oops {{ no close", &v), "oops {{ no close");
}

#[test]
fn allows_dotted_and_underscored_keys() {
    let v = vars(&[("a.b", "AB"), ("my_var", "X")]);
    assert_eq!(substitute("{{a.b}}/{{my_var}}", &v), "AB/X");
}
