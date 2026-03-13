use crate::config_loader::AppConfig;
use crate::trace_store::TraceStore;

pub fn execute_replay(config: &AppConfig, target: &str) -> Result<(), String> {
    let store = TraceStore::from_workspace_root(config.workspace.clone())?;

    let (summary, latest) = if target == "latest" {
        let summary = store.replay_summary()?;
        let latest = store
            .latest_event()?
            .ok_or_else(|| String::from("replay target not found: latest"))?;
        (summary, latest)
    } else {
        let summary = store
            .replay_summary_for_intent(target)?
            .ok_or_else(|| format!("replay target not found: {target}"))?;
        let latest = store
            .latest_event_for_intent(target)?
            .ok_or_else(|| format!("replay target not found: {target}"))?;
        (summary, latest)
    };

    println!(
        "replay intent_id={} count={} revision={} mode={} kind={} outcome={} policy={}",
        latest.intent_id,
        summary.intent_count,
        summary.latest_revision,
        summary.latest_mode,
        latest.kind,
        latest.outcome,
        latest.policy_code,
    );
    println!(
        "replay stages provider={} memory={} tool={} report_written={}",
        latest.provider, latest.memory, latest.tool, latest.report_written,
    );
    println!(
        "replay verification status={} summary={}",
        latest.verification.status, latest.verification.summary,
    );
    println!(
        "replay artifacts plan={} apply={} verify={} report={}",
        latest.artifacts.plan,
        latest.artifacts.apply,
        latest.artifacts.verify,
        latest.artifacts.report,
    );
    for patch in &latest.patch_artifacts {
        println!(
            "replay patch target={} artifact={} before={} after={}",
            patch.target_path,
            patch.artifact_path,
            patch.before_digest.as_deref().unwrap_or("none"),
            patch.after_digest,
        );
    }
    if let Some(failure) = &latest.first_failure {
        println!(
            "replay failure stage={} message={}",
            failure.stage, failure.message
        );
    }
    println!("replay summary failed_intents={}", summary.failed_intents);

    Ok(())
}
