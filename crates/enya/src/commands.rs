use enya_workspace::{WorkspaceConfig, list_workspaces, resolve_workspace_path, workspace_dir};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

pub fn init(
    name: Option<String>,
    endpoint: Option<&str>,
    template: Option<&str>,
    output: Option<&str>,
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
    println!("Created {}", path.display());
    Ok(())
}

pub fn list() -> Result {
    let dir = workspace_dir();
    let workspaces = list_workspaces();

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

pub fn show(name: &str) -> Result {
    let path = resolve_workspace_path(name);
    let ws = WorkspaceConfig::load(&path)?;

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

pub fn rm(name: &str) -> Result {
    let path = resolve_workspace_path(name);
    if !path.exists() {
        return Err(format!("workspace not found: {}", path.display()).into());
    }

    std::fs::remove_file(&path)?;
    println!("Removed {}", path.display());
    Ok(())
}
