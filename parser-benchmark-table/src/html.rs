use crate::model::{ParserSelection, RepoSelection, RowResult};
use std::collections::BTreeSet;
use std::path::Path;

pub fn build_html(
    rows: &[RowResult],
    parser_selection: &ParserSelection,
    repo_selection: &RepoSelection,
) -> String {
    let mut rows_html = String::new();

    let show_native = matches!(
        parser_selection,
        ParserSelection::NativeOnly | ParserSelection::Both
    );
    let show_conjure = matches!(
        parser_selection,
        ParserSelection::ViaConjureOnly | ParserSelection::Both
    );

    let mut header_cells = String::new();
    if show_native {
        header_cells.push_str("        <th class=\"col-native\">Native</th>\n");
    }
    if show_conjure {
        header_cells.push_str("        <th class=\"col-via\">Via-Conjure</th>\n");
    }

    let detail_colspan = 1 + usize::from(show_native) + usize::from(show_conjure);

    let mut repos = BTreeSet::new();
    for row in rows {
        repos.insert(row.repo_name.clone());
    }

    let mut repo_options = String::new();
    for repo in repos {
        repo_options.push_str(&format!(
            "        <option value=\"{}\">{}</option>\n",
            escape_html_attr(&repo),
            escape_html(&repo)
        ));
    }

    for (idx, row) in rows.iter().enumerate() {
        let details_id = format!("details-{}", idx);

        rows_html.push_str(&format!(
            "<tr class=\"summary-row\" data-detail-id=\"{}\" data-repo=\"{}\" data-native-status=\"{}\" data-via-status=\"{}\" data-test-name=\"{}\" data-primary-path=\"{}\" data-companion-path=\"{}\" onclick=\"toggleDetails('{}')\">\n",
            escape_html_attr(&details_id),
            escape_html_attr(&row.repo_name),
            escape_html_attr(match &row.native {
                Some(p) if p.pass => "pass",
                Some(_) => "fail",
                None => "not-run",
            }),
            escape_html_attr(match &row.via_conjure {
                Some(p) if p.pass => "pass",
                Some(_) => "fail",
                None => "not-run",
            }),
            escape_html_attr(&row.test_name),
            escape_html_attr(&row.primary_relative),
            escape_html_attr(&row.param_relative),
            escape_html_attr(&details_id)
        ));

        rows_html.push_str(&format!(
            "<td class=\"summary-name\">{}</td>\n",
            escape_html(&row.test_name)
        ));

        if show_native {
            if let Some(native) = &row.native {
                let native_class = if native.pass { "pass" } else { "fail" };
                rows_html.push_str(&format!(
                    "<td class=\"col-native {}\">{}</td>\n",
                    native_class,
                    escape_html(native.summary)
                ));
            } else {
                rows_html.push_str("<td class=\"col-native\">not run</td>\n");
            }
        }

        if show_conjure {
            if let Some(conjure) = &row.via_conjure {
                let conjure_class = if conjure.pass { "pass" } else { "fail" };
                rows_html.push_str(&format!(
                    "<td class=\"col-conjure {}\">{}</td>\n",
                    conjure_class,
                    escape_html(conjure.summary)
                ));
            } else {
                rows_html.push_str("<td class=\"col-conjure\">not run</td>\n");
            }
        }

        rows_html.push_str("</tr>\n");
        rows_html.push_str(&format!(
            "<tr id=\"{}\" class=\"details-row\"><td colspan=\"{}\">\n",
            escape_html_attr(&details_id),
            detail_colspan
        ));

        rows_html.push_str(&format!(
            "<div class=\"meta\"><strong>Repo:</strong> {} | <strong>Kind:</strong> {}</div>\n",
            escape_html(&row.repo_name),
            escape_html(row.kind)
        ));
        rows_html.push_str(&format!(
            "<div class=\"meta\"><strong>Input file:</strong> {}</div>\n",
            escape_html(&row.primary_relative)
        ));
        if !row.param_relative.is_empty() {
            rows_html.push_str(&format!(
                "<div class=\"meta\"><strong>Param file:</strong> {}</div>\n",
                escape_html(&row.param_relative)
            ));
        }

        rows_html.push_str("<div class=\"details-block\"><div class=\"details-title\">Primary input contents</div>");
        rows_html.push_str(&format!(
            "<div class=\"mono input-content\">{}</div></div>\n",
            escape_html(&row.primary_contents)
        ));

        if !row.param_contents.is_empty() {
            rows_html.push_str(
                "<div class=\"details-block\"><div class=\"details-title\">Param file contents</div>",
            );
            rows_html.push_str(&format!(
                "<div class=\"mono input-content\">{}</div></div>\n",
                escape_html(&row.param_contents)
            ));
        }

        if let Some(native) = &row.native {
            rows_html.push_str("<div class=\"details-block detail-native\"><div class=\"details-title\">Native parser output</div>");
            rows_html.push_str(&format!(
                "<div class=\"mono\">{}</div></div>\n",
                escape_html(&native.output_or_error)
            ));
        }

        if let Some(via) = &row.via_conjure {
            rows_html.push_str("<div class=\"details-block detail-via\"><div class=\"details-title\">Via-conjure parser output</div>");
            rows_html.push_str(&format!(
                "<div class=\"mono\">{}</div></div>\n",
                escape_html(&via.output_or_error)
            ));
        }

        rows_html.push_str("</td></tr>\n");
    }

    let template = include_str!("template.html");
    template
        .replace("__TOTAL__", &rows.len().to_string())
        .replace(
            "__RUN_INFO__",
            &run_info_text(parser_selection, repo_selection),
        )
        .replace("__REPO_OPTIONS__", &repo_options)
        .replace("__HEADER_CELLS__", &header_cells)
        .replace("__ROWS__", &rows_html)
}

pub fn derive_test_name(repo_name: &str, primary_relative: &str) -> String {
    if repo_name == "conjure-oxide" {
        let p = Path::new(primary_relative);
        let components: Vec<String> = p
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        if let Some(pos) = components.iter().position(|c| c == "tests") {
            let start = pos + 1;
            if start < components.len() {
                return components[start..].join("/");
            }
        }
    }

    primary_relative.to_string()
}

fn run_info_text(parser_selection: &ParserSelection, repo_selection: &RepoSelection) -> String {
    let parser_text = match parser_selection {
        ParserSelection::NativeOnly => "native",
        ParserSelection::ViaConjureOnly => "via-conjure",
        ParserSelection::Both => "native + via-conjure",
    };

    let mut repos = Vec::new();
    if repo_selection.conjure_oxide {
        repos.push("conjure-oxide");
    }
    if repo_selection.conjure {
        repos.push("conjure");
    }
    if repo_selection.essence_catalog {
        repos.push("EssenceCatalog");
    }

    format!(
        "Parsers: {} | Repositories: {}",
        parser_text,
        repos.join(", ")
    )
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_html_attr(input: &str) -> String {
    escape_html(input).replace('\n', " ")
}
