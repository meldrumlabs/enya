use enya_workspace::{
    PaneConfig, SectionConfig, SectionLayout, WorkspaceConfig, list_workspaces,
    resolve_workspace_path, workspace_dir,
};

use crate::Result;

pub fn init(
    name: Option<String>,
    endpoint: Option<&str>,
    template: Option<&str>,
    output: Option<&str>,
    json: bool,
) -> Result {
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
            let toml_str = match t {
                "default" => enya_workspace::DEFAULT_WORKSPACE_TOML,
                "demo" => enya_workspace::DEMO_WORKSPACE_TOML,
                "complex" => enya_workspace::COMPLEX_VIEWPORT_TOML,
                "atlas" => enya_workspace::ATLAS_WORKSPACE_TOML,
                _ => {
                    return Err(format!(
                        "unknown template: {t} (available: default, demo, complex, atlas)"
                    )
                    .into());
                }
            };
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

    if json {
        println!(
            "{}",
            serde_json::json!({"name": name, "path": path.display().to_string()})
        );
    } else {
        println!("Created {}", path.display());
    }
    Ok(())
}

pub fn list(json: bool) -> Result {
    let dir = workspace_dir();
    let workspaces = list_workspaces();

    if json {
        let items: Vec<_> = workspaces
            .iter()
            .map(|(name, desc)| {
                serde_json::json!({
                    "name": name,
                    "description": desc,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({"dir": dir.display().to_string(), "workspaces": items})
        );
        return Ok(());
    }

    if workspaces.is_empty() {
        println!("No workspaces found in {}", dir.display());
        return Ok(());
    }

    println!("Workspaces in {}:\n", dir.display());
    for (name, description) in &workspaces {
        match description {
            Some(desc) => println!("  {name:20} {desc}"),
            None => println!("  {name}"),
        }
    }
    Ok(())
}

pub fn show(name: &str, json: bool) -> Result {
    let path = resolve_workspace_path(name);
    let ws = WorkspaceConfig::load(&path)?;

    if json {
        println!("{}", serde_json::to_string(&ws)?);
        return Ok(());
    }

    println!("Name:        {}", ws.workspace.name);
    if !ws.workspace.description.is_empty() {
        println!("Description: {}", ws.workspace.description);
    }
    if let Some(ep) = ws.effective_endpoint() {
        println!("Endpoint:    {ep}");
    }
    if !ws.logs.is_empty() {
        println!("Logs:        {}", ws.logs.endpoint);
    }
    if !ws.git.is_empty() {
        println!("Git:         {}", ws.git.url);
    }

    println!("Theme:       {}", ws.view.theme);
    println!("Time:        {}", ws.time.preset);

    if ws.uses_sections() {
        println!("Sections:    {}", ws.sections.len());
        for (i, section) in ws.sections.iter().enumerate() {
            let collapsed = if section.collapsed {
                " (collapsed)"
            } else {
                ""
            };
            println!(
                "  [{i}] {} ({} panes, {:?}){collapsed}",
                section.name,
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
            println!("Panes:       {} (legacy format)", all_panes.len());
            for (i, pane) in all_panes.iter().enumerate() {
                let label = if pane.name.is_empty() {
                    format!("pane {i}")
                } else {
                    pane.name.clone()
                };
                println!("  [{i}] {label}: {}", pane.query);
            }
        }
    }

    Ok(())
}

pub fn rm(name: &str, json: bool) -> Result {
    let path = resolve_workspace_path(name);
    if !path.exists() {
        return Err(format!("workspace not found: {}", path.display()).into());
    }

    std::fs::remove_file(&path)?;

    if json {
        println!(
            "{}",
            serde_json::json!({"removed": path.display().to_string()})
        );
    } else {
        println!("Removed {}", path.display());
    }
    Ok(())
}

// -- Property access ----------------------------------------------------------

pub fn get(name: &str, key: &str, json: bool) -> Result {
    let path = resolve_workspace_path(name);
    let ws = WorkspaceConfig::load(&path)?;
    let value = ws.get_value(key)?;

    if json {
        println!(
            "{}",
            serde_json::json!({"workspace": ws.workspace.name, "key": key, "value": value})
        );
    } else {
        println!("{value}");
    }
    Ok(())
}

pub fn set(name: &str, key: &str, value: &str, json: bool) -> Result {
    let path = resolve_workspace_path(name);
    let mut ws = WorkspaceConfig::load(&path)?;
    ws.set_value(key, value)?;
    ws.save(&path)?;

    if json {
        println!(
            "{}",
            serde_json::json!({"workspace": ws.workspace.name, "key": key, "value": value})
        );
    } else {
        println!("{key} = {value}");
    }
    Ok(())
}

// -- Section/pane mutations ---------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn add_pane(
    name: &str,
    query: &str,
    pane_name: Option<&str>,
    section: Option<&str>,
    tag: Option<&str>,
    unit: Option<&str>,
    granularity: Option<&str>,
    visualization: Option<&str>,
    description: Option<&str>,
    json: bool,
) -> Result {
    let path = resolve_workspace_path(name);
    let mut ws = WorkspaceConfig::load(&path)?;

    ws.ensure_default_section();

    let section_idx = if let Some(sec_name) = section {
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

    let mut pane = PaneConfig::new(query);
    if let Some(n) = pane_name {
        pane.name = n.to_string();
    }
    if let Some(t) = tag {
        pane.tag = t.to_string();
    }
    if let Some(u) = unit {
        pane.unit = u.to_string();
    }
    if let Some(g) = granularity {
        pane.granularity = g.to_string();
    }
    if let Some(v) = visualization {
        pane.visualization = v.to_string();
    }
    if let Some(d) = description {
        pane.description = d.to_string();
    }

    let sec_name = ws.sections[section_idx].name.clone();
    ws.sections[section_idx].panes.push(pane);
    ws.save(&path)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "workspace": ws.workspace.name,
                "section": sec_name,
                "pane": pane_name.unwrap_or(""),
                "query": query,
            })
        );
    } else {
        let label = pane_name.unwrap_or(query);
        println!("Added pane \"{label}\" to section \"{sec_name}\"");
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

    if json {
        println!(
            "{}",
            serde_json::json!({
                "workspace": ws.workspace.name,
                "section": section_name,
                "layout": layout,
            })
        );
    } else {
        println!("Added section \"{section_name}\" ({layout})");
    }
    Ok(())
}

pub fn remove_pane(name: &str, pane: &str, section: Option<&str>, json: bool) -> Result {
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

    if json {
        println!(
            "{}",
            serde_json::json!({
                "workspace": ws.workspace.name,
                "removed_pane": pane,
                "section": sec_name,
            })
        );
    } else {
        println!("Removed pane \"{pane}\" from section \"{sec_name}\"");
    }
    Ok(())
}

pub fn remove_section(name: &str, section_name: &str, json: bool) -> Result {
    let path = resolve_workspace_path(name);
    let mut ws = WorkspaceConfig::load(&path)?;

    let idx = ws
        .find_section(section_name)
        .ok_or_else(|| format!("section not found: {section_name}"))?;

    let panes_removed = ws.sections[idx].panes.len();
    ws.sections.remove(idx);
    ws.save(&path)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "workspace": ws.workspace.name,
                "removed_section": section_name,
                "panes_removed": panes_removed,
            })
        );
    } else {
        println!("Removed section \"{section_name}\" ({panes_removed} panes)");
    }
    Ok(())
}
