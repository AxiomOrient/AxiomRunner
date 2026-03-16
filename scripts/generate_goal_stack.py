#!/usr/bin/env python3
import argparse
import json
import re
from pathlib import Path
from typing import Optional


PRESETS = {
    "rust-service": {
        "workflow_pack": "examples/rust_service/pack.json",
        "verification_checks": [
            {"label": "build", "detail": "cargo build"},
            {"label": "test", "detail": "cargo test"},
            {"label": "lint", "detail": "cargo clippy -- -D warnings"},
        ],
        "budget": {"max_steps": 8, "max_minutes": 15, "max_tokens": 12000},
    },
    "node-api": {
        "workflow_pack": "examples/node_api/pack.json",
        "verification_checks": [
            {"label": "install", "detail": "npm install --ignore-scripts"},
            {"label": "lint", "detail": "npm run lint"},
            {"label": "test", "detail": "npm test"},
            {"label": "build", "detail": "npm run build"},
        ],
        "budget": {"max_steps": 10, "max_minutes": 15, "max_tokens": 12000},
    },
    "nextjs-app": {
        "workflow_pack": "examples/nextjs_app/pack.json",
        "verification_checks": [
            {"label": "lint", "detail": "npm run lint"},
            {"label": "typecheck", "detail": "npm run typecheck"},
            {"label": "test", "detail": "npm test"},
            {"label": "build", "detail": "npm run build"},
        ],
        "budget": {"max_steps": 10, "max_minutes": 15, "max_tokens": 12000},
    },
    "python-fastapi": {
        "workflow_pack": "examples/python_fastapi/pack.json",
        "verification_checks": [
            {"label": "format", "detail": "python3 -m compileall app.py"},
            {"label": "test", "detail": "python3 -m unittest -q"},
            {"label": "import smoke", "detail": "python3 app.py"},
        ],
        "budget": {"max_steps": 9, "max_minutes": 15, "max_tokens": 12000},
    },
}

DEFAULT_BUDGET = {"max_steps": 8, "max_minutes": 15, "max_tokens": 12000}
DEFAULT_APPROVAL_MODE = "never"


def slugify(value: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    return slug or "goal"


def load_json(path: Path):
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def ensure_dir(path: Path):
    path.mkdir(parents=True, exist_ok=True)


def unique_ordered_constraints(base_constraints, slice_paths, extra_constraints):
    constraints = []
    if slice_paths:
        constraints.append(
            {"label": "path_scope", "detail": ",".join(slice_paths)}
        )
    for bucket in (base_constraints, extra_constraints):
        for item in bucket:
            if item not in constraints:
                constraints.append(item)
    return constraints


def build_done_conditions(slice_data):
    done_conditions = []
    for index, text in enumerate(slice_data["acceptance"], start=1):
        done_conditions.append(
            {
                "label": f"acceptance-{index}",
                "evidence": text,
            }
        )
    return done_conditions


def resolve_preset(name: Optional[str]):
    if not name:
        return {}
    if name not in PRESETS:
        allowed = ", ".join(sorted(PRESETS))
        raise ValueError(f"unknown preset '{name}'. expected one of: {allowed}")
    return PRESETS[name]


def _first_of(*candidates):
    return next((c for c in candidates if c), None)


def choose_budget(top_level_budget, preset_budget, slice_budget):
    return _first_of(slice_budget, top_level_budget, preset_budget) or DEFAULT_BUDGET


def choose_verifiers(top_level_verifiers, preset_verifiers, slice_verifiers):
    result = _first_of(slice_verifiers, top_level_verifiers, preset_verifiers)
    if not result:
        raise ValueError("each slice needs verification_checks or a preset with defaults")
    return result


def render_stack_markdown(spec, generated):
    lines = [
        f"# Goal Stack: {spec['epic']}",
        "",
        f"- workspace_root: `{spec['workspace_root']}`",
        f"- preset: `{spec.get('preset', 'custom')}`",
        f"- generated_goals: `{len(generated)}`",
    ]
    if spec.get("independence_rules"):
        lines.extend(["", "## Independence Rules"])
        lines.extend(f"- {rule}" for rule in spec["independence_rules"])

    lines.extend(["", "## Slices"])
    for item in generated:
        lines.extend(
            [
                "",
                f"### {item['index']:02d}. {item['slice']['summary']}",
                f"- goal_file: `{item['path'].name}`",
                f"- why: {item['slice'].get('why', 'not provided')}",
                f"- paths: `{','.join(item['slice']['paths'])}`",
                f"- workflow_pack: `{item['goal'].get('workflow_pack', 'none')}`",
                f"- verifier_labels: `{','.join(check['label'] for check in item['goal']['verification_checks'])}`",
                "- acceptance:",
            ]
        )
        lines.extend(f"  - {text}" for text in item["slice"]["acceptance"])
    lines.append("")
    return "\n".join(lines)


def build_goal(spec, slice_data):
    preset = resolve_preset(slice_data.get("preset") or spec.get("preset"))
    workflow_pack = (
        slice_data.get("workflow_pack")
        or spec.get("workflow_pack")
        or preset.get("workflow_pack")
    )
    verification_checks = choose_verifiers(
        spec.get("verification_checks"),
        preset.get("verification_checks"),
        slice_data.get("verification_checks"),
    )
    budget = choose_budget(
        spec.get("budget"),
        preset.get("budget"),
        slice_data.get("budget"),
    )
    constraints = unique_ordered_constraints(
        spec.get("constraints", []),
        slice_data["paths"],
        slice_data.get("constraints", []),
    )
    goal = {
        "summary": slice_data["summary"],
        "workspace_root": slice_data.get("workspace_root", spec["workspace_root"]),
        "constraints": constraints,
        "done_conditions": build_done_conditions(slice_data),
        "verification_checks": verification_checks,
        "budget": budget,
        "approval_mode": slice_data.get(
            "approval_mode",
            spec.get("approval_mode", DEFAULT_APPROVAL_MODE),
        ),
    }
    if workflow_pack:
        goal["workflow_pack"] = workflow_pack
    return goal


def validate_spec(spec):
    for field in ("epic", "workspace_root", "slices"):
        if field not in spec:
            raise ValueError(f"missing required field '{field}'")
    if not isinstance(spec["slices"], list) or not spec["slices"]:
        raise ValueError("slices must be a non-empty array")

    seen_ids = set()
    for slice_data in spec["slices"]:
        for field in ("id", "summary", "paths", "acceptance"):
            if field not in slice_data:
                raise ValueError(f"slice missing required field '{field}'")
        if slice_data["id"] in seen_ids:
            raise ValueError(f"duplicate slice id '{slice_data['id']}'")
        seen_ids.add(slice_data["id"])
        if not slice_data["paths"]:
            raise ValueError(f"slice '{slice_data['id']}' needs at least one path")
        if not slice_data["acceptance"]:
            raise ValueError(f"slice '{slice_data['id']}' needs at least one acceptance item")


def write_json(path: Path, payload):
    with path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2)
        handle.write("\n")


def main():
    parser = argparse.ArgumentParser(
        description="Generate atomic AxiomRunner goal files from a smaller stack brief."
    )
    parser.add_argument("brief", help="Path to the goal stack brief JSON file")
    parser.add_argument(
        "--output-dir",
        help="Directory to write generated goal files into",
        required=True,
    )
    args = parser.parse_args()

    brief_path = Path(args.brief).resolve()
    output_dir = Path(args.output_dir).resolve()
    ensure_dir(output_dir)

    spec = load_json(brief_path)
    validate_spec(spec)

    generated = []
    for index, slice_data in enumerate(spec["slices"], start=1):
        goal = build_goal(spec, slice_data)
        file_name = f"{index:02d}_{slugify(slice_data['id'])}.goal.json"
        path = output_dir / file_name
        write_json(path, goal)
        generated.append(
            {"index": index, "slice": slice_data, "goal": goal, "path": path}
        )

    stack_path = output_dir / "GOAL_STACK.md"
    stack_path.write_text(render_stack_markdown(spec, generated), encoding="utf-8")

    print(
        json.dumps(
            {
                "brief": str(brief_path),
                "output_dir": str(output_dir),
                "generated_goals": [item["path"].name for item in generated],
                "stack": stack_path.name,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
