use crate::config_loader::AppConfig;
use crate::operator_render::render_replay_lines;
use crate::runtime_compose::RuntimeComposeConfig;
use crate::trace_store::TraceStore;

pub fn execute_replay(config: &AppConfig, target: &str) -> Result<(), String> {
    let compose_config = RuntimeComposeConfig::from_app_config(config);
    let store = TraceStore::from_workspace_root(
        compose_config
            .artifact_workspace
            .or(compose_config.tool_workspace),
    )?;

    let (summary, latest, artifact_index) = if target == "latest" {
        let summary = store.replay_summary()?;
        let latest = store
            .latest_event()?
            .ok_or_else(|| String::from("replay target not found: latest"))?;
        let artifact_index = store.artifact_index()?;
        (summary, latest, artifact_index)
    } else if let Some(summary) = store.replay_summary_for_intent(target)? {
        let latest = store
            .latest_event_for_intent(target)?
            .ok_or_else(|| format!("replay target not found: {target}"))?;
        let latest_entry = store
            .artifact_index_for_intent(target)?
            .ok_or_else(|| format!("replay target not found: {target}"))?;
        (
            summary,
            latest,
            crate::trace_store::TraceArtifactIndex {
                entries: vec![latest_entry],
            },
        )
    } else if let Some(summary) = store.replay_summary_for_run(target)? {
        let latest = store
            .latest_event_for_run(target)?
            .ok_or_else(|| format!("replay target not found: {target}"))?;
        let latest_entry = store
            .artifact_index_for_run(target)?
            .ok_or_else(|| format!("replay target not found: {target}"))?;
        (
            summary,
            latest,
            crate::trace_store::TraceArtifactIndex {
                entries: vec![latest_entry],
            },
        )
    } else {
        return Err(format!("replay target not found: {target}"));
    };

    for line in render_replay_lines(&latest, &summary, &artifact_index) {
        println!("{line}");
    }

    Ok(())
}
