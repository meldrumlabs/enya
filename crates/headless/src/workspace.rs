use console::style;
use enya_workspace::{
    PaneConfig, SectionConfig, SectionLayout, WorkspaceConfig, list_workspaces,
    resolve_workspace_path, workspace_dir,
};
use serde::Serialize;

use crate::Result;
use crate::query::{promql, time};

// -- Result types -------------------------------------------------------------

#[derive(Serialize)]
pub struct InitResult {
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct ListResult {
    pub dir: String,
    pub workspaces: Vec<WorkspaceEntry>,
}

#[derive(Serialize)]
pub struct WorkspaceEntry {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct RmResult {
    pub removed: String,
}

#[derive(Serialize)]
pub struct GetResult {
    pub workspace: String,
    pub key: String,
    pub value: String,
}

#[derive(Serialize)]
pub struct SetResult {
    pub workspace: String,
    pub key: String,
    pub value: String,
}

#[derive(Serialize)]
pub struct AddSectionResult {
    pub workspace: String,
    pub section: String,
    pub layout: String,
}

#[derive(Serialize)]
pub struct AddPaneResult {
    pub workspace: String,
    pub section: String,
    pub pane: String,
    pub query: String,
}

#[derive(Serialize)]
pub struct RemoveSectionResult {
    pub workspace: String,
    pub removed_section: String,
    pub panes_removed: usize,
}

#[derive(Serialize)]
pub struct RemovePaneResult {
    pub workspace: String,
    pub removed_pane: String,
    pub section: String,
}

/// Parameters for adding a pane to a workspace.
pub struct AddPaneParams<'a> {
    pub name: &'a str,
    pub query: &'a str,
    pub pane_name: Option<&'a str>,
    pub section: Option<&'a str>,
    pub tag: Option<&'a str>,
    pub unit: Option<&'a str>,
    pub granularity: Option<&'a str>,
    pub visualization: Option<&'a str>,
    pub description: Option<&'a str>,
}

// -- Template resolution ------------------------------------------------------

/// Resolve a template name to its TOML content.
pub fn resolve_template(template: &str) -> Result<&'static str> {
    match template {
        "default" => Ok(enya_workspace::DEFAULT_WORKSPACE_TOML),
        "demo" => Ok(enya_workspace::DEMO_WORKSPACE_TOML),
        "complex" => Ok(enya_workspace::COMPLEX_VIEWPORT_TOML),
        "atlas" => Ok(enya_workspace::ATLAS_WORKSPACE_TOML),
        _ => Err(format!(
            "unknown template: {template} (available: default, demo, complex, atlas)"
        )
        .into()),
    }
}

// -- Core functions (return data, no printing) --------------------------------

pub fn init_core(
    name: Option<String>,
    endpoint: Option<&str>,
    template: Option<&str>,
    output: Option<&str>,
) -> Result<InitResult> {
    let name = name.unwrap_or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "workspace".to_string())
    });

    let path = match output {
        Some(o) => std::path::PathBuf::from(o),
        None => workspace_dir().join(format!("{name}.toml")),
    };

    if path.exists() {
        return Err(format!("{} already exists", path.display()).into());
    }

    let ws = match template {
        Some(t) => {
            let toml_str = resolve_template(t)?;
            let mut ws = WorkspaceConfig::from_toml(toml_str)?;
            ws.workspace.name = name.clone();
            if let Some(ep) = endpoint {
                ws.workspace.endpoint = ep.to_string();
            }
            ws
        }
        None => match endpoint {
            Some(ep) => WorkspaceConfig::with_endpoint(&name, ep),
            None => WorkspaceConfig::new(&name),
        },
    };

    ws.save(&path)?;
    Ok(InitResult {
        name,
        path: path.display().to_string(),
    })
}

pub fn list_core() -> ListResult {
    let dir = workspace_dir();
    let workspaces = list_workspaces();
    ListResult {
        dir: dir.display().to_string(),
        workspaces: workspaces
            .into_iter()
            .map(|(name, description)| WorkspaceEntry { name, description })
            .collect(),
    }
}

pub fn show_core(name: &str) -> Result<WorkspaceConfig> {
    let path = resolve_workspace_path(name);
    Ok(WorkspaceConfig::load(&path)?)
}

pub fn rm_core(name: &str) -> Result<RmResult> {
    let path = resolve_workspace_path(name);
    if !path.exists() {
        return Err(format!("workspace not found: {}", path.display()).into());
    }
    std::fs::remove_file(&path)?;
    Ok(RmResult {
        removed: path.display().to_string(),
    })
}

pub fn get_core(name: &str, key: &str) -> Result<GetResult> {
    let path = resolve_workspace_path(name);
    let ws = WorkspaceConfig::load(&path)?;
    let value = ws.get_value(key)?;
    Ok(GetResult {
        workspace: ws.workspace.name,
        key: key.to_string(),
        value,
    })
}

pub fn set_core(name: &str, key: &str, value: &str) -> Result<SetResult> {
    let path = resolve_workspace_path(name);
    let mut ws = WorkspaceConfig::load(&path)?;
    ws.set_value(key, value)?;
    ws.save(&path)?;
    Ok(SetResult {
        workspace: ws.workspace.name,
        key: key.to_string(),
        value: value.to_string(),
    })
}

pub fn add_section_core(
    name: &str,
    section_name: &str,
    layout: &str,
    columns: Option<usize>,
    collapsed: bool,
) -> Result<AddSectionResult> {
    let path = resolve_workspace_path(name);
    let mut ws = WorkspaceConfig::load(&path)?;

    if ws.find_section(section_name).is_some() {
        return Err(format!("section already exists: {section_name}").into());
    }

    let section_layout = SectionLayout::parse(layout).ok_or_else(|| {
        format!("invalid layout: {layout} (expected: horizontal, vertical, grid, tabs)")
    })?;

    let mut section = SectionConfig::new(section_name).with_layout(section_layout);
    if let Some(cols) = columns {
        section = section.with_columns(cols);
    }
    if collapsed {
        section = section.with_collapsed(true);
    }

    ws.add_section(section);
    ws.save(&path)?;
    Ok(AddSectionResult {
        workspace: ws.workspace.name,
        section: section_name.to_string(),
        layout: layout.to_string(),
    })
}

pub fn add_pane_core(params: &AddPaneParams<'_>) -> Result<AddPaneResult> {
    let path = resolve_workspace_path(params.name);
    let mut ws = WorkspaceConfig::load(&path)?;

    ws.ensure_default_section();

    let section_idx = if let Some(sec_name) = params.section {
        ws.find_section(sec_name).ok_or_else(|| {
            let available: Vec<&str> = ws.sections.iter().map(|s| s.name.as_str()).collect();
            format!(
                "section not found: {sec_name} (available: {})",
                available.join(", ")
            )
        })?
    } else {
        ws.sections.len() - 1
    };

    let mut pane = PaneConfig::new(params.query);
    if let Some(n) = params.pane_name {
        pane.name = n.to_string();
    }
    if let Some(t) = params.tag {
        pane.tag = t.to_string();
    }
    if let Some(u) = params.unit {
        pane.unit = u.to_string();
    }
    if let Some(g) = params.granularity {
        pane.granularity = g.to_string();
    }
    if let Some(v) = params.visualization {
        pane.visualization = v.to_string();
    }
    if let Some(d) = params.description {
        pane.description = d.to_string();
    }

    let sec_name = ws.sections[section_idx].name.clone();
    ws.sections[section_idx].panes.push(pane);
    ws.save(&path)?;

    Ok(AddPaneResult {
        workspace: ws.workspace.name,
        section: sec_name,
        pane: params.pane_name.unwrap_or("").to_string(),
        query: params.query.to_string(),
    })
}

pub fn remove_section_core(name: &str, section_name: &str) -> Result<RemoveSectionResult> {
    let path = resolve_workspace_path(name);
    let mut ws = WorkspaceConfig::load(&path)?;

    let idx = ws
        .find_section(section_name)
        .ok_or_else(|| format!("section not found: {section_name}"))?;

    let panes_removed = ws.sections[idx].panes.len();
    ws.sections.remove(idx);
    ws.save(&path)?;

    Ok(RemoveSectionResult {
        workspace: ws.workspace.name,
        removed_section: section_name.to_string(),
        panes_removed,
    })
}

pub fn remove_pane_core(name: &str, pane: &str, section: Option<&str>) -> Result<RemovePaneResult> {
    let path = resolve_workspace_path(name);
    let mut ws = WorkspaceConfig::load(&path)?;

    let matches = if let Some(sec_name) = section {
        let si = ws
            .find_section(sec_name)
            .ok_or_else(|| format!("section not found: {sec_name}"))?;
        ws.sections[si]
            .panes
            .iter()
            .enumerate()
            .filter(|(_, p)| p.name == pane)
            .map(|(pi, _)| (si, pi))
            .collect::<Vec<_>>()
    } else {
        ws.find_pane_by_name(pane)
    };

    if matches.is_empty() {
        return Err(format!("pane not found: {pane}").into());
    }
    if matches.len() > 1 {
        let sections: Vec<&str> = matches
            .iter()
            .map(|(si, _)| ws.sections[*si].name.as_str())
            .collect();
        return Err(format!(
            "multiple panes named \"{pane}\" (in sections: {}). Use --section to disambiguate.",
            sections.join(", ")
        )
        .into());
    }

    let (si, pi) = matches[0];
    let sec_name = ws.sections[si].name.clone();
    ws.sections[si].panes.remove(pi);
    ws.save(&path)?;

    Ok(RemovePaneResult {
        workspace: ws.workspace.name,
        removed_pane: pane.to_string(),
        section: sec_name,
    })
}

// -- CLI wrappers (call core + format output) ---------------------------------

pub fn init(
    name: Option<String>,
    endpoint: Option<&str>,
    template: Option<&str>,
    output: Option<&str>,
    json: bool,
) -> Result {
    let result = init_core(name, endpoint, template, output)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("{} {}", style("Created").green(), result.path);
    }
    Ok(())
}

pub fn list(json: bool) -> Result {
    let result = list_core();

    if json {
        println!("{}", serde_json::to_string(&result)?);
        return Ok(());
    }

    if result.workspaces.is_empty() {
        println!("No workspaces found in {}", result.dir);
        return Ok(());
    }

    println!(
        "{}\n",
        style(format!("Workspaces in {}:", result.dir)).bold()
    );
    for entry in &result.workspaces {
        match &entry.description {
            Some(desc) => println!("  {:20} {}", style(&entry.name).bold(), style(desc).dim()),
            None => println!("  {}", style(&entry.name).bold()),
        }
    }
    Ok(())
}

pub fn show(name: &str, json: bool) -> Result {
    let ws = show_core(name)?;

    if json {
        println!("{}", serde_json::to_string(&ws)?);
        return Ok(());
    }

    println!("{} {}", style("Name:").bold(), ws.workspace.name);
    if !ws.workspace.description.is_empty() {
        println!(
            "{} {}",
            style("Description:").bold(),
            ws.workspace.description
        );
    }
    if let Some(ep) = ws.effective_endpoint() {
        println!("{} {ep}", style("Endpoint:").bold());
    }
    if !ws.logs.is_empty() {
        println!("{} {}", style("Logs:").bold(), ws.logs.endpoint);
    }
    if !ws.git.is_empty() {
        println!("{} {}", style("Git:").bold(), ws.git.url);
    }

    println!("{} {}", style("Theme:").bold(), ws.view.theme);
    println!("{} {}", style("Time:").bold(), ws.time.preset);

    if ws.uses_sections() {
        println!("{} {}", style("Sections:").bold(), ws.sections.len());
        for (i, section) in ws.sections.iter().enumerate() {
            let collapsed = if section.collapsed {
                " (collapsed)"
            } else {
                ""
            };
            println!(
                "  {} {} ({} panes, {:?}){collapsed}",
                style(format!("[{i}]")).dim(),
                style(&section.name).bold(),
                section.panes.len(),
                section.layout,
            );
            for pane in &section.panes {
                let label = if pane.name.is_empty() {
                    &pane.query
                } else {
                    &pane.name
                };
                println!("      - {label}: {}", pane.query);
            }
        }
    } else {
        let all_panes = ws.all_panes();
        if !all_panes.is_empty() {
            println!(
                "{} {} (legacy format)",
                style("Panes:").bold(),
                all_panes.len()
            );
            for (i, pane) in all_panes.iter().enumerate() {
                let label = if pane.name.is_empty() {
                    format!("pane {i}")
                } else {
                    pane.name.clone()
                };
                println!(
                    "  {} {label}: {}",
                    style(format!("[{i}]")).dim(),
                    pane.query
                );
            }
        }
    }

    Ok(())
}

pub fn rm(name: &str, json: bool) -> Result {
    let result = rm_core(name)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("{} {}", style("Removed").green(), result.removed);
    }
    Ok(())
}

pub fn get(name: &str, key: &str, json: bool) -> Result {
    let result = get_core(name, key)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("{}", result.value);
    }
    Ok(())
}

pub fn set(name: &str, key: &str, value: &str, json: bool) -> Result {
    let result = set_core(name, key, value)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("{} = {}", style(&result.key).bold(), result.value);
    }
    Ok(())
}

pub fn add_section(
    name: &str,
    section_name: &str,
    layout: &str,
    columns: Option<usize>,
    collapsed: bool,
    json: bool,
) -> Result {
    let result = add_section_core(name, section_name, layout, columns, collapsed)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!(
            "{} section \"{}\" ({})",
            style("Added").green(),
            result.section,
            result.layout
        );
    }
    Ok(())
}

pub fn add_pane(params: &AddPaneParams<'_>, json: bool) -> Result {
    let result = add_pane_core(params)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        let label = params.pane_name.unwrap_or(params.query);
        println!(
            "{} pane \"{label}\" to section \"{}\"",
            style("Added").green(),
            result.section
        );
    }
    Ok(())
}

pub fn remove_section(name: &str, section_name: &str, json: bool) -> Result {
    let result = remove_section_core(name, section_name)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!(
            "{} section \"{}\" ({} panes)",
            style("Removed").green(),
            result.removed_section,
            result.panes_removed
        );
    }
    Ok(())
}

pub fn remove_pane(name: &str, pane: &str, section: Option<&str>, json: bool) -> Result {
    let result = remove_pane_core(name, pane, section)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!(
            "{} pane \"{}\" from section \"{}\"",
            style("Removed").green(),
            result.removed_pane,
            result.section
        );
    }
    Ok(())
}

// -- Snapshot -----------------------------------------------------------------

/// Choose a reasonable query step/resolution based on the time range.
fn compute_step(range_secs: u64) -> u64 {
    match range_secs {
        0..=900 => 15,        // ≤15m → 15s step
        901..=3600 => 60,     // ≤1h  → 1m step
        3601..=21600 => 300,  // ≤6h  → 5m step
        21601..=86400 => 900, // ≤1d  → 15m step
        _ => 3600,            // >1d  → 1h step
    }
}

/// Capture a point-in-time snapshot of all pane query results in a workspace.
///
/// For each pane, executes the query as a range query over the workspace's
/// configured time preset and returns the results as a self-contained JSON blob.
pub fn snapshot(base_url: &str, ws: &WorkspaceConfig) -> Result<serde_json::Value> {
    let now = time::now_secs();
    let preset = &ws.time.preset;
    let range_secs = time::parse_duration_secs(if preset.is_empty() { "1h" } else { preset })?;
    let start_secs = now.saturating_sub(range_secs);
    let step_secs = compute_step(range_secs);

    let mut pane_results: Vec<serde_json::Value> = Vec::new();

    for section in &ws.sections {
        for pane in &section.panes {
            let result =
                match promql::query_range(base_url, &pane.query, start_secs, now, step_secs) {
                    Ok(data) => serde_json::to_value(&data)
                        .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})),
                    Err(e) => serde_json::json!({"error": e.to_string()}),
                };

            pane_results.push(serde_json::json!({
                "section": section.name,
                "name": pane.name,
                "query": pane.query,
                "result": result,
            }));
        }
    }

    Ok(serde_json::json!({
        "version": 1,
        "captured_at": now,
        "captured_at_human": time::format_timestamp(now as f64),
        "time_range": {
            "start": start_secs,
            "end": now,
            "step": step_secs,
            "preset": preset,
        },
        "workspace": serde_json::to_value(ws)?,
        "panes": pane_results,
    }))
}

/// CLI entry point for `enya snapshot`.
pub fn snapshot_cmd(
    name: &str,
    endpoint: Option<&str>,
    output: Option<&str>,
    json: bool,
) -> Result {
    let path = resolve_workspace_path(name);
    let ws = WorkspaceConfig::load(&path)?;

    let base_url = promql::resolve_endpoint(endpoint, Some(name))?;

    let snap = snapshot(&base_url, &ws)?;

    if let Some(out_path) = output {
        let contents = serde_json::to_string_pretty(&snap)?;
        std::fs::write(out_path, contents)?;
        if json {
            println!("{}", serde_json::json!({"path": out_path}));
        } else {
            let pane_count = snap["panes"].as_array().map(|a| a.len()).unwrap_or(0);
            println!("Snapshot written to {out_path} ({pane_count} panes)");
        }
    } else if json {
        println!("{}", serde_json::to_string(&snap)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&snap)?);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_step() {
        assert_eq!(compute_step(300), 15); // 5m range → 15s step
        assert_eq!(compute_step(900), 15); // 15m range → 15s step
        assert_eq!(compute_step(3600), 60); // 1h range → 1m step
        assert_eq!(compute_step(21600), 300); // 6h range → 5m step
        assert_eq!(compute_step(86400), 900); // 1d range → 15m step
        assert_eq!(compute_step(604800), 3600); // 7d range → 1h step
    }

    #[test]
    fn test_resolve_template() {
        assert!(resolve_template("default").is_ok());
        assert!(resolve_template("demo").is_ok());
        assert!(resolve_template("complex").is_ok());
        assert!(resolve_template("atlas").is_ok());
        assert!(resolve_template("nonexistent").is_err());
    }
}
