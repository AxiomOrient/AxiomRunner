use crate::cli_command::{CliCommand, IntentTemplate};
use crate::config_loader::AppConfig;
use crate::display::mode_name;
use crate::runtime_compose::{RuntimeComposeConfig, RuntimeComposeState};
use crate::status::{
    RuntimeStatusInput, StateStatusInput, StatusInput, StatusSnapshot, render_status_lines,
};
use axonrunner_core::{
    AgentState, DecisionOutcome, DomainEvent, Intent, IntentKind, PolicyCode, build_policy_audit,
    decide, evaluate_policy, project_from,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppliedIntent {
    intent_id: String,
    kind: &'static str,
    outcome: DecisionOutcome,
    policy_code: PolicyCode,
    effect_count: usize,
}

pub struct CliRuntime {
    state: AgentState,
    actor_id: String,
    next_intent_seq: u64,
    compose_state: RuntimeComposeState,
}

impl CliRuntime {
    pub fn new(actor_id: String, config: &AppConfig) -> Result<Self, String> {
        Self::new_with_compose(
            actor_id,
            RuntimeComposeConfig::from_env(config.provider.as_str()),
        )
    }

    fn new_with_compose(
        actor_id: String,
        compose_config: RuntimeComposeConfig,
    ) -> Result<Self, String> {
        Ok(Self {
            state: AgentState::default(),
            actor_id,
            next_intent_seq: 0,
            compose_state: RuntimeComposeState::new(compose_config)?,
        })
    }

    fn reset(&mut self) {
        self.state = AgentState::default();
        self.next_intent_seq = 0;
        if let Err(error) = self.compose_state.clear() {
            eprintln!("runtime_compose clear failed error={error}");
        }
    }

    fn apply_template(&mut self, template: &IntentTemplate) -> AppliedIntent {
        let intent = template.to_intent(self.next_intent_id(), Some(self.actor_id.clone()));
        self.apply_intent(intent)
    }

    fn apply_intent(&mut self, intent: Intent) -> AppliedIntent {
        let verdict = evaluate_policy(&self.state, &intent);
        let decision = decide(&intent, &verdict);
        let audit = build_policy_audit(&self.state, &intent, &verdict);
        let effects = decision.effects.clone();

        let events = vec![
            DomainEvent::IntentAccepted {
                intent: intent.clone(),
            },
            DomainEvent::PolicyEvaluated { audit },
            DomainEvent::DecisionCalculated { decision },
            DomainEvent::EffectsApplied { effects },
        ];

        self.state = project_from(&self.state, &events);

        let decision = match &events[2] {
            DomainEvent::DecisionCalculated { decision } => decision,
            _ => unreachable!("event order is fixed"),
        };
        let audit = match &events[1] {
            DomainEvent::PolicyEvaluated { audit } => audit,
            _ => unreachable!("event order is fixed"),
        };
        let effects = match &events[3] {
            DomainEvent::EffectsApplied { effects } => effects,
            _ => unreachable!("event order is fixed"),
        };

        AppliedIntent {
            intent_id: intent.intent_id,
            kind: intent_kind_name(&intent.kind),
            outcome: decision.outcome,
            policy_code: audit.code,
            effect_count: effects.len(),
        }
    }

    fn persist_template_result(&mut self, template: &IntentTemplate, applied: &AppliedIntent) {
        let execution =
            self.compose_state
                .apply_template(template, &applied.intent_id, applied.outcome);
        if let Some((stage, message)) = execution.first_failure() {
            eprintln!(
                "runtime_compose failed intent_id={} stage={} error={}",
                applied.intent_id, stage, message
            );
        }
    }

    fn next_intent_id(&mut self) -> String {
        self.next_intent_seq = self.next_intent_seq.saturating_add(1);
        format!("cli-{}", self.next_intent_seq)
    }
}

pub fn execute_command(
    runtime: &mut CliRuntime,
    config: &AppConfig,
    command: CliCommand,
) -> Result<(), String> {
    match command {
        CliCommand::Run(intent) => execute_intent(runtime, &intent),
        CliCommand::Batch {
            intents,
            reset_state,
        } => {
            if reset_state {
                runtime.reset();
            }
            for intent in &intents {
                execute_intent(runtime, intent);
            }
            print_summary("batch", intents.len(), &runtime.state);
        }
        CliCommand::Status => print_status(runtime),
        CliCommand::Health => print_health(&runtime.state, config),
    }

    Ok(())
}

fn execute_intent(runtime: &mut CliRuntime, intent: &IntentTemplate) {
    if let IntentTemplate::Read { key } = intent {
        print_read_value(&runtime.state, key);
        return;
    }

    let applied = runtime.apply_template(intent);
    runtime.persist_template_result(intent, &applied);
    print_intent_result(&applied);
}

fn print_intent_result(applied: &AppliedIntent) {
    println!(
        "intent id={} kind={} outcome={} policy={} effects={}",
        applied.intent_id,
        applied.kind,
        outcome_name(applied.outcome),
        applied.policy_code.as_str(),
        applied.effect_count
    );
}

fn print_read_value(state: &AgentState, key: &str) {
    let value = state.facts.get(key).map(String::as_str).unwrap_or("<none>");
    println!("read key={} value={}", key, value);
}

fn print_summary(label: &str, intent_count: usize, state: &AgentState) {
    println!(
        "{} completed count={} revision={} mode={} facts={} denied={} audit={}",
        label,
        intent_count,
        state.revision,
        mode_name(state.mode),
        state.facts.len(),
        state.denied_count,
        state.audit_count
    );
}

fn print_status(runtime: &CliRuntime) {
    let compose = runtime.compose_state.health();
    let snapshot = StatusSnapshot::from(StatusInput {
        state: StateStatusInput {
            revision: runtime.state.revision,
            mode: runtime.state.mode,
            facts: runtime.state.facts.len(),
            denied: runtime.state.denied_count,
            audit: runtime.state.audit_count,
        },
        runtime: RuntimeStatusInput {
            provider_id: compose.provider_id,
            provider_model: compose.provider_model,
            memory_enabled: compose.memory.enabled,
            memory_state: compose.memory.state.to_string(),
            tool_enabled: compose.tool.enabled,
            tool_state: compose.tool.state.to_string(),
        },
    });

    for line in render_status_lines(&snapshot) {
        println!("{line}");
    }
}

fn print_health(state: &AgentState, config: &AppConfig) {
    println!(
        "health ok=true profile={} mode={} revision={}",
        config.profile,
        mode_name(state.mode),
        state.revision
    );
}

fn intent_kind_name(kind: &IntentKind) -> &'static str {
    match kind {
        IntentKind::ReadFact { .. } => "read",
        IntentKind::WriteFact { .. } => "write",
        IntentKind::RemoveFact { .. } => "remove",
        IntentKind::FreezeWrites => "freeze",
        IntentKind::Halt => "halt",
    }
}

fn outcome_name(outcome: DecisionOutcome) -> &'static str {
    match outcome {
        DecisionOutcome::Accepted => "accepted",
        DecisionOutcome::Rejected => "rejected",
    }
}
