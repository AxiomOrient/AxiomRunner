use crate::config_loader::AppConfig;
use crate::trace_store::TraceStore;

pub fn execute_replay(config: &AppConfig, target: &str) -> Result<(), String> {
    let store = TraceStore::from_workspace_root(config.workspace.clone())?;

    let (summary, latest, artifact_index) = if target == "latest" {
        let summary = store.replay_summary()?;
        let latest = store
            .latest_event()?
            .ok_or_else(|| String::from("replay target not found: latest"))?;
        let artifact_index = store.artifact_index()?;
        (summary, latest, artifact_index)
    } else {
        if let Some(summary) = store.replay_summary_for_intent(target)? {
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
        }
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
    if let Some(run) = &latest.run {
        println!(
            "replay run run_id={} phase={} outcome={} reason={} planned_steps={} summary={}",
            run.run_id, run.phase, run.outcome, run.reason, run.planned_steps, run.plan_summary
        );
        println!(
            "replay repair attempted={} status={} summary={} step_ids={}",
            run.repair.attempted,
            run.repair.status,
            run.repair.summary,
            if run.step_ids.is_empty() {
                String::from("none")
            } else {
                run.step_ids.join(",")
            }
        );
        for step in &run.step_journal {
            println!(
                "replay step id={} phase={} status={} label={} evidence={} failure={}",
                step.id,
                step.phase,
                step.status,
                step.label,
                step.evidence,
                step.failure.as_deref().unwrap_or("none")
            );
        }
    }
    println!(
        "replay artifacts plan={} apply={} verify={} report={}",
        latest.artifacts.plan,
        latest.artifacts.apply,
        latest.artifacts.verify,
        latest.artifacts.report,
    );
    println!(
        "replay artifact_index count={} latest_report={}",
        artifact_index.entries.len(),
        artifact_index
            .entries
            .last()
            .map(|entry| entry.report.as_str())
            .unwrap_or("none")
    );
    if !latest.patch_artifacts.is_empty() {
        let changed_paths = latest
            .patch_artifacts
            .iter()
            .map(|patch| patch.target_path.as_str())
            .collect::<Vec<_>>();
        println!(
            "replay changed_paths count={} paths={}",
            changed_paths.len(),
            changed_paths.join(",")
        );
    }
    for patch in &latest.patch_artifacts {
        println!(
            "replay patch target={} op={} artifact={} before={} after={}",
            patch.target_path,
            patch.operation,
            patch.artifact_path,
            patch.before_digest.as_deref().unwrap_or("none"),
            patch.after_digest.as_deref().unwrap_or("none"),
        );
        if let Some(excerpt) = &patch.before_excerpt {
            println!("replay patch before_excerpt={excerpt}");
        }
        if let Some(excerpt) = &patch.after_excerpt {
            println!("replay patch after_excerpt={excerpt}");
        }
        if let Some(diff) = &patch.unified_diff {
            println!("replay patch unified_diff={diff}");
        }
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
