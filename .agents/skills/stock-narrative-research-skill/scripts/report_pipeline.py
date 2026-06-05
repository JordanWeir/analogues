#!/usr/bin/env python3
"""Orchestrate stock narrative report runs.

The pipeline creates a ticker-specific working directory, scaffolds section JSON
files for agent-authored analysis, runs deterministic scenario math, validates
the bundle, compiles canonical report data, and renders a static HTML artifact.
"""

from __future__ import annotations

import argparse
import html
import json
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
SKILL_DIR = SCRIPT_DIR.parent
WORKSPACE_DIR = next(
    (
        parent
        for parent in SKILL_DIR.parents
        if (parent / ".agents").exists() or (parent / ".git").exists()
    ),
    SKILL_DIR,
)
OUTPUTS_DIR = WORKSPACE_DIR / "reports" / "stock-narrative-research"
TEMPLATE_PATH = SKILL_DIR / "templates" / "report.html.j2"
PROJECTION_NOTE = (
    "These scenario projections are illustrative and assumption-driven. They are not "
    "predictions, price targets, or investment advice. They show how different narrative "
    "outcomes could translate into financial assumptions and valuation ranges."
)

SECTION_FILES = {
    "company": "company.json",
    "sources": "sources.json",
    "claims": "claims.json",
    "orientation": "orientation.json",
    "business_model": "business-model.json",
    "why_now": "why-now.json",
    "narrative_map": "narrative-map.json",
    "financial_snapshot": "financial-snapshot.json",
    "financial_math": "financial-math.json",
    "scenario_assumptions": "scenario-assumptions.json",
    "industry_context": "industry-context.json",
    "watch_items": "watch-items.json",
    "historical_analogues": "historical-analogues.json",
    "final_talk_track": "final-talk-track.json",
    "limitations": "limitations.json",
}

def main() -> int:
    parser = argparse.ArgumentParser(description="Run stock narrative report pipeline steps.")
    parser.add_argument("ticker", help="Ticker symbol, for example MSFT")
    parser.add_argument(
        "command",
        choices=["init", "fetch", "seed-scenarios", "project", "validate", "compile", "render", "all"],
        help="Pipeline command to run.",
    )
    parser.add_argument(
        "--workdir",
        help="Existing run directory as an absolute path or a path relative to the workspace root.",
    )
    parser.add_argument("--force", action="store_true", help="Overwrite an existing initialized run directory.")
    args = parser.parse_args()

    ticker = args.ticker.upper()
    if args.command == "init":
        workdir = init_run(ticker, args.workdir, args.force)
        print(workdir)
        return 0

    workdir = resolve_workdir(ticker, args.workdir)
    if args.command == "fetch":
        fetch_run(ticker, workdir)
    elif args.command == "seed-scenarios":
        seed_scenario_assumptions(workdir)
    elif args.command == "project":
        project_run(workdir)
    elif args.command == "validate":
        return validate_run(workdir)
    elif args.command == "compile":
        compile_run(workdir)
    elif args.command == "render":
        render_run(workdir)
    elif args.command == "all":
        fetch_run(ticker, workdir)
        seed_scenario_assumptions(workdir)
        project_run(workdir)
        validation_code = validate_run(workdir)
        if validation_code != 0:
            return validation_code
        compile_run(workdir)
        render_run(workdir)
    return 0


def init_run(ticker: str, workdir_arg: str | None, force: bool) -> Path:
    workdir = Path(workdir_arg) if workdir_arg else OUTPUTS_DIR / f"{ticker}-{timestamp_slug()}"
    if not workdir.is_absolute():
        workdir = WORKSPACE_DIR / workdir
    if workdir.exists():
        if not force:
            raise SystemExit(f"Run directory already exists: {workdir}. Use --force to reinitialize.")
        shutil.rmtree(workdir)

    (workdir / "raw").mkdir(parents=True)
    (workdir / "sections").mkdir()
    (workdir / "generated").mkdir()

    write_json(
        workdir / "manifest.json",
        {
            "ticker": ticker,
            "created_at": now_iso(),
            "projection_note": PROJECTION_NOTE,
            "status": {
                "initialized": True,
                "fetched": False,
                "projected": False,
                "validated": False,
                "compiled": False,
                "rendered": False,
            },
            "notes": [
                "Agent-authored analysis belongs in sections/*.json.",
                "Raw fetched data belongs in raw/.",
                "Generated outputs belong in generated/.",
            ],
        },
    )

    for key, filename in SECTION_FILES.items():
        write_json(workdir / "sections" / filename, default_section(key, ticker))
    return workdir


def fetch_run(ticker: str, workdir: Path) -> None:
    ensure_run(workdir)
    sys.path.insert(0, str(SCRIPT_DIR))
    from fetch_financials import fetch_snapshot  # noqa: WPS433

    snapshot = fetch_snapshot(ticker)
    write_json(workdir / "raw" / "financials.json", snapshot)
    seed_from_financials(workdir, snapshot)
    seed_scenario_assumptions(workdir)
    update_manifest(workdir, "fetched")
    print(f"Wrote {workdir / 'raw' / 'financials.json'}")


def project_run(workdir: Path) -> None:
    ensure_run(workdir)
    sys.path.insert(0, str(SCRIPT_DIR))
    from scenario_calculator import build_projection  # noqa: WPS433

    assumptions_path = workdir / "sections" / "scenario-assumptions.json"
    assumptions = read_json(assumptions_path)
    try:
        scenario_data = build_projection(assumptions)
    except Exception as exc:
        raise SystemExit(f"Scenario calculation failed: {exc}") from exc

    scenario_path = workdir / "generated" / "scenario-data.json"
    review_path = workdir / "generated" / "scenario-review.json"
    write_json(scenario_path, scenario_data)
    write_json(review_path, review_scenarios(scenario_data))
    update_manifest(workdir, "projected")
    print(f"Wrote {scenario_path}")
    print(f"Wrote {review_path}")


def validate_run(workdir: Path) -> int:
    ensure_run(workdir)
    errors: list[str] = []
    warnings: list[str] = []
    sections = load_sections(workdir, errors)

    if not has_filled_item(sections.get("sources"), ["source", "type", "date", "why_it_matters"]):
        errors.append("sections/sources.json needs at least one filled source.")
    if not has_filled_item(sections.get("claims"), ["claim", "source", "date", "type", "side", "confidence"]):
        errors.append("sections/claims.json needs at least one filled claim.")

    for key in ("orientation", "business_model", "why_now", "narrative_map", "financial_math", "industry_context", "final_talk_track"):
        if is_blank(sections.get(key)):
            errors.append(f"sections/{SECTION_FILES[key]} is still blank.")

    assumptions = sections.get("scenario_assumptions") or {}
    baseline = assumptions.get("baseline") if isinstance(assumptions, dict) else {}
    if not is_number((baseline or {}).get("revenue")):
        errors.append("sections/scenario-assumptions.json baseline.revenue must be numeric before project/render.")
    if not is_number((baseline or {}).get("diluted_shares")):
        errors.append("sections/scenario-assumptions.json baseline.diluted_shares must be numeric before project/render.")
    if not assumptions.get("scenarios"):
        errors.append("sections/scenario-assumptions.json needs scenario assumptions.")
    elif isinstance(assumptions.get("scenarios"), list):
        scenario_count = len(assumptions["scenarios"])
        if scenario_count < 4 or scenario_count > 6:
            warnings.append("sections/scenario-assumptions.json should contain 4-6 crux-derived scenarios.")
        stances = {
            str(scenario.get("stance", "")).lower()
            for scenario in assumptions["scenarios"]
            if isinstance(scenario, dict) and scenario.get("stance")
        }
        missing_stances = {"bullish", "neutral", "bearish"} - stances
        if missing_stances:
            warnings.append(
                "sections/scenario-assumptions.json should include at least one bullish, neutral, and bearish stance; "
                f"missing: {', '.join(sorted(missing_stances))}."
            )
        probability_total = 0.0
        missing_probability_count = 0
        for index, scenario in enumerate(assumptions["scenarios"], start=1):
            name = scenario.get("name") or f"scenario {index}"
            if not scenario.get("description"):
                errors.append(f"Scenario {name!r} needs a narrative description.")
            if not scenario.get("stance"):
                warnings.append(f"Scenario {name!r} should set stance to bullish, neutral, bearish, or mixed.")
            probability = scenario.get("probability")
            if probability is None:
                missing_probability_count += 1
                warnings.append(f"Scenario {name!r} should include a conditional probability for Monte Carlo weighting.")
            elif is_number(probability) and probability >= 0:
                probability_total += probability
            else:
                errors.append(f"Scenario {name!r} probability must be a non-negative number.")
            if not scenario.get("crux_assumptions"):
                warnings.append(f"Scenario {name!r} should explain how narrative cruxes are settled in crux_assumptions.")
            if not scenario.get("periods"):
                errors.append(f"Scenario {name!r} needs at least one period assumption.")
        if missing_probability_count == 0 and probability_total > 0 and abs(probability_total - 1.0) > 0.01:
            warnings.append(f"Scenario probabilities sum to {probability_total:.3f}; the calculator will normalize them for Monte Carlo sampling.")

    scenario_path = workdir / "generated" / "scenario-data.json"
    if scenario_path.exists():
        scenario_data = read_json(scenario_path)
        if scenario_data.get("projection_note") != PROJECTION_NOTE:
            errors.append("generated/scenario-data.json has a missing or modified projection note.")
        warnings.extend(review_scenarios(scenario_data).get("warnings", []))
    else:
        warnings.append("generated/scenario-data.json is missing. Run the project step after scenario assumptions are filled.")

    result = {"errors": errors, "warnings": warnings, "validated_at": now_iso()}
    write_json(workdir / "generated" / "validation.json", result)
    update_manifest(workdir, "validated", value=not errors)

    for item in errors:
        print(f"ERROR: {item}", file=sys.stderr)
    for item in warnings:
        print(f"WARNING: {item}", file=sys.stderr)
    if errors:
        return 1
    print("Validation passed.")
    return 0


def compile_run(workdir: Path) -> None:
    ensure_run(workdir)
    errors: list[str] = []
    sections = load_sections(workdir, errors)
    if errors:
        raise SystemExit("\n".join(errors))

    scenario_path = workdir / "generated" / "scenario-data.json"
    if not scenario_path.exists():
        raise SystemExit("Missing generated/scenario-data.json. Run project first.")
    scenario_data = read_json(scenario_path)

    report_data = {
        "company": sections["company"],
        "generated_at": now_iso(),
        "projection_note": PROJECTION_NOTE,
        "source_pack": sections["sources"],
        "claim_table": sections["claims"],
        "financial_snapshot": sections["financial_snapshot"],
        "sections": {
            "orientation": sections["orientation"],
            "business_model": sections["business_model"],
            "why_now": sections["why_now"],
            "narrative_map": sections["narrative_map"],
            "financial_math": sections["financial_math"],
            "scenario_projection_summary": summarize_scenarios(scenario_data),
            "industry_context": sections["industry_context"],
            "final_talk_track": sections["final_talk_track"],
        },
        "historical_analogues": sections["historical_analogues"],
        "watch_items": sections["watch_items"],
        "source_notes_and_limitations": sections["limitations"],
        "scenario_data": scenario_data,
    }
    report_path = workdir / "generated" / "report-data.json"
    write_json(report_path, report_data)
    update_manifest(workdir, "compiled")
    print(f"Wrote {report_path}")


def render_run(workdir: Path) -> None:
    ensure_run(workdir)
    report_path = workdir / "generated" / "report-data.json"
    if not report_path.exists():
        raise SystemExit("Missing generated/report-data.json. Run compile first.")
    report_data = read_json(report_path)
    html_text = render_template(report_data)
    output_path = workdir / "generated" / "report.html"
    output_path.write_text(html_text, encoding="utf-8")
    update_manifest(workdir, "rendered")
    print(f"Wrote {output_path}")


def default_section(key: str, ticker: str) -> Any:
    if key == "company":
        return {"ticker": ticker, "name": "", "exchange": "", "currency": "USD", "sector": "", "industry": ""}
    if key == "sources":
        return [
            {
                "source": "",
                "url": "",
                "type": "Official company source",
                "date": "",
                "why_it_matters": "",
                "claims_supported": [],
            }
        ]
    if key == "claims":
        return [
            {
                "claim": "",
                "source": "",
                "date": "",
                "type": "demand",
                "side": "bull",
                "confidence": "Medium",
                "related_metric": "",
            }
        ]
    if key == "watch_items":
        return [
            {
                "signal": "",
                "why_it_matters": "",
                "scenario_affected": "",
                "bull_signal": "",
                "bear_signal": "",
            }
        ]
    if key == "historical_analogues":
        return [
            {
                "analogue": "",
                "narrative_type": "",
                "why_similar": "",
                "how_it_played_out": "",
                "financial_pattern": "",
                "key_pivots": [],
                "lesson": "",
                "why_misleading": "",
                "source_notes": "",
            }
        ]
    if key == "narrative_map":
        return {
            "dominant": "",
            "bull": "",
            "bear": "",
            "consensus": "",
            "counter_narrative": "",
            "agreements": [],
            "cruxes": [],
        }
    if key == "scenario_assumptions":
        return {
            "company": "",
            "ticker": ticker,
            "currency": "USD",
            "current_price": None,
            "base_year": "",
            "baseline": {"revenue": None, "diluted_shares": None, "net_margin": None, "eps": None},
            "scenario_generation_guidance": {
                "scenario_count": "Create 4-6 company-specific scenarios after narrative_map.cruxes are filled.",
                "stance_coverage": "Include at least one bullish, one neutral, and one bearish scenario.",
                "naming": "Use crux-derived scenario names, not fixed generic buckets.",
                "probabilities": "Assign a probability to each conditional scenario. Probabilities should usually sum to 1.0 before normalization.",
                "simulation": "The project step runs a 10,000-iteration Monte Carlo by default, using each scenario's terminal low/median/high price band as a rough P10/P50/P90 normal distribution.",
                "transparency": "For each scenario, fill probability, stance, crux_assumptions, confirming_signals, breaking_signals, revenue/margin/EPS assumptions, and P/S or P/E multiple bands.",
            },
            "monte_carlo": {"iterations": 10000, "seed": 42, "bins": 30},
            "scenarios": [],
            "source_notes": [],
        }
    if key == "orientation":
        return {
            "company_plain_english": "",
            "dominant_question": "",
            "time_horizon": "",
            "current_setup": "",
            "base_rate_warning": "",
        }
    if key == "business_model":
        return {
            "what_the_company_sells": "",
            "how_it_makes_money": "",
            "key_growth_drivers": "",
            "key_constraints": "",
            "capital_intensity": "",
        }
    if key == "why_now":
        return {
            "results_inflection": "",
            "narrative_inflection": "",
            "scarcity_or_demand_signal": "",
            "risk_signal": "",
            "next_catalysts": "",
        }
    if key == "financial_snapshot":
        return {
            "current_share_price": "",
            "market_cap": "",
            "ttm_revenue": "",
            "ttm_eps": "",
            "gross_margin": "",
            "net_margin": "",
            "cash": "",
            "total_debt": "",
            "source_note": "",
        }
    if key == "financial_math":
        return []
    if key == "industry_context":
        return {
            "market_structure": "",
            "demand_drivers": "",
            "supply_constraints": "",
            "competitive_position": "",
            "customer_power": "",
            "regulatory_or_geopolitical": "",
        }
    if key == "final_talk_track":
        return {
            "one_minute_version": "",
            "bull_case": "",
            "bear_case": "",
            "what_would_change_the_narrative_upward": "",
            "what_would_change_the_narrative_downward": "",
            "projection_framing": "Scenario-conditioned implied ranges are assumption-driven and are not predictions or investment advice.",
        }
    if key == "limitations":
        return {"source_gaps": [], "data_limitations": [], "analogue_limitations": [], "projection_limitations": []}
    return {}


def seed_from_financials(workdir: Path, snapshot: dict[str, Any]) -> None:
    company_path = workdir / "sections" / "company.json"
    company = read_json(company_path)
    if snapshot.get("company_name") and not company.get("name"):
        company["name"] = snapshot["company_name"]
    if snapshot.get("currency"):
        company["currency"] = snapshot["currency"]
    write_json(company_path, company)

    assumptions_path = workdir / "sections" / "scenario-assumptions.json"
    assumptions = read_json(assumptions_path)
    assumptions["company"] = assumptions.get("company") or snapshot.get("company_name") or ""
    assumptions["currency"] = snapshot.get("currency") or assumptions.get("currency") or "USD"
    assumptions["current_price"] = snapshot.get("current_price") if snapshot.get("current_price") is not None else assumptions.get("current_price")
    baseline = assumptions.setdefault("baseline", {})
    baseline["revenue"] = baseline.get("revenue") if baseline.get("revenue") is not None else snapshot.get("revenue_ttm")
    baseline["diluted_shares"] = baseline.get("diluted_shares") if baseline.get("diluted_shares") is not None else snapshot.get("shares_outstanding")
    baseline["net_margin"] = baseline.get("net_margin") if baseline.get("net_margin") is not None else snapshot.get("net_margin")
    baseline["eps"] = baseline.get("eps") if baseline.get("eps") is not None else snapshot.get("eps_ttm")
    source_notes = assumptions.setdefault("source_notes", [])
    for note in snapshot.get("source_notes", []):
        if note not in source_notes:
            source_notes.append(note)
    write_json(assumptions_path, assumptions)

    financial_path = workdir / "sections" / "financial-snapshot.json"
    financial_snapshot = read_json(financial_path)
    seeded_snapshot = seeded_financial_snapshot(snapshot)
    for key, value in seeded_snapshot.items():
        if is_blank(financial_snapshot.get(key)) and value is not None:
            financial_snapshot[key] = value
    write_json(financial_path, financial_snapshot)


def seed_scenario_assumptions(workdir: Path) -> None:
    """Seed baseline and period scaffolding from raw financials without overwriting analysis."""
    assumptions_path = workdir / "sections" / "scenario-assumptions.json"
    assumptions = read_json(assumptions_path)
    financials_path = workdir / "raw" / "financials.json"
    snapshot = read_json(financials_path) if financials_path.exists() else {}

    assumptions["company"] = assumptions.get("company") or snapshot.get("company_name") or ""
    assumptions["currency"] = snapshot.get("currency") or assumptions.get("currency") or "USD"
    if assumptions.get("current_price") is None:
        assumptions["current_price"] = snapshot.get("current_price")
    if not assumptions.get("base_year"):
        assumptions["base_year"] = snapshot.get("fundamental_period_end") or "Latest available public financials"

    baseline = assumptions.setdefault("baseline", {})
    fill_if_missing(baseline, "revenue", snapshot.get("revenue_ttm"))
    fill_if_missing(baseline, "diluted_shares", snapshot.get("shares_outstanding"))
    fill_if_missing(baseline, "net_margin", snapshot.get("net_margin"))
    fill_if_missing(baseline, "eps", snapshot.get("eps_ttm"))

    shares = baseline.get("diluted_shares")
    for scenario in assumptions.get("scenarios", []):
        if not isinstance(scenario, dict):
            continue
        periods = scenario.get("periods")
        if not periods:
            scenario["periods"] = scenario_period_templates(shares)
        elif isinstance(periods, list):
            for period in periods:
                if isinstance(period, dict) and period.get("diluted_shares") is None and shares is not None:
                    period["diluted_shares"] = shares

    source_notes = assumptions.setdefault("source_notes", [])
    note = "Scenario baseline seeded from raw/financials.json; scenario narratives, growth, margins, and multiple bands still require analyst judgment."
    if note not in source_notes:
        source_notes.append(note)
    for snapshot_note in snapshot.get("source_notes", []):
        if snapshot_note not in source_notes:
            source_notes.append(snapshot_note)

    write_json(assumptions_path, assumptions)


def scenario_period_templates(diluted_shares: float | None = None) -> list[dict[str, Any]]:
    return [
        scenario_period_template("+12 months", diluted_shares),
        scenario_period_template("+24 months", diluted_shares),
        scenario_period_template("+36 months", diluted_shares),
    ]


def scenario_period_template(label: str, diluted_shares: float | None) -> dict[str, Any]:
    return {
        "label": label,
        "revenue": None,
        "revenue_growth": None,
        "net_margin": None,
        "diluted_shares": diluted_shares,
        "ps_multiple": {"low": None, "median": None, "high": None},
        "pe_multiple": {"low": None, "median": None, "high": None},
        "blend_weights": {"ps": 0.5, "pe": 0.5},
    }


def seeded_financial_snapshot(snapshot: dict[str, Any]) -> dict[str, str | None]:
    return {
        "current_share_price": format_optional_money(snapshot.get("current_price")),
        "market_cap": format_optional_money(snapshot.get("market_cap")),
        "ttm_revenue": format_optional_money(snapshot.get("revenue_ttm")),
        "ttm_eps": format_optional_money(snapshot.get("eps_ttm")),
        "gross_margin": format_optional_percent(snapshot.get("gross_margin")),
        "net_margin": format_optional_percent(snapshot.get("net_margin")),
        "cash": format_optional_money(snapshot.get("cash")),
        "total_debt": format_optional_money(snapshot.get("total_debt")),
        "source_note": "; ".join(snapshot.get("source_notes", [])) if snapshot.get("source_notes") else None,
    }


def fill_if_missing(payload: dict[str, Any], key: str, value: Any) -> None:
    if payload.get(key) is None and value is not None:
        payload[key] = value


def review_scenarios(scenario_data: dict[str, Any]) -> dict[str, Any]:
    warnings: list[str] = []
    current_price = scenario_data.get("current_price")
    terminal_lows: list[float] = []
    terminal_highs: list[float] = []
    scenarios = scenario_data.get("scenarios", [])

    if len(scenarios) < 4 or len(scenarios) > 6:
        warnings.append("Scenario set should usually contain 4-6 crux-derived scenarios.")
    stances = {
        str(scenario.get("stance", "")).lower()
        for scenario in scenarios
        if isinstance(scenario, dict) and scenario.get("stance")
    }
    missing_stances = {"bullish", "neutral", "bearish"} - stances
    if missing_stances:
        warnings.append(f"Scenario set is missing stance coverage: {', '.join(sorted(missing_stances))}.")
    probability_total = 0.0
    missing_probability_count = 0

    for scenario in scenarios:
        probability = scenario.get("probability")
        if probability is None:
            missing_probability_count += 1
        elif is_number(probability) and probability >= 0:
            probability_total += probability
        else:
            warnings.append(f"Scenario {scenario.get('name', '<unnamed>')} has an invalid probability.")
        if not scenario.get("crux_assumptions"):
            warnings.append(f"Scenario {scenario.get('name', '<unnamed>')} should include crux_assumptions.")
        periods = scenario.get("periods", [])
        if not periods:
            warnings.append(f"Scenario {scenario.get('name', '<unnamed>')} has no periods.")
            continue
        for period in periods:
            band = period.get("blended_price") or period.get("ps_implied_price") or period.get("pe_implied_price")
            if not band:
                warnings.append(f"Scenario {scenario.get('name')} period {period.get('label')} has no price band.")
                continue
            low, median, high = band.get("low"), band.get("median"), band.get("high")
            if all(is_number(value) for value in (low, median, high)) and not (low <= median <= high):
                warnings.append(f"Scenario {scenario.get('name')} period {period.get('label')} band is not ordered low <= median <= high.")
        terminal_band = (periods[-1].get("blended_price") or periods[-1].get("ps_implied_price") or {})
        if is_number(terminal_band.get("low")):
            terminal_lows.append(terminal_band["low"])
        if is_number(terminal_band.get("high")):
            terminal_highs.append(terminal_band["high"])

    if missing_probability_count:
        warnings.append("Scenario set has missing probabilities; Monte Carlo sampling will use equal weights only if no positive probabilities are supplied.")
    elif probability_total > 0 and abs(probability_total - 1.0) > 0.01:
        warnings.append(f"Scenario probabilities sum to {probability_total:.3f}; Monte Carlo sampling normalizes them.")
    monte_carlo = scenario_data.get("monte_carlo") or {}
    if not monte_carlo.get("histogram"):
        warnings.append("Monte Carlo histogram is missing or empty.")
    if is_number(current_price) and terminal_lows and terminal_highs:
        if current_price < min(terminal_lows) or current_price > max(terminal_highs):
            warnings.append("Current price sits outside all terminal scenario bands; review assumptions and multiple ranges.")
    return {"warnings": warnings, "reviewed_at": now_iso()}


def summarize_scenarios(scenario_data: dict[str, Any]) -> dict[str, str]:
    summary: dict[str, str] = {}
    for scenario in scenario_data.get("scenarios", []):
        periods = scenario.get("periods", [])
        if not periods:
            continue
        terminal = periods[-1]
        band = terminal.get("blended_price") or terminal.get("ps_implied_price") or terminal.get("pe_implied_price") or {}
        key = slug_key(scenario.get("name", "scenario"))
        summary[key] = (
            f"{terminal.get('label', 'terminal')} illustrative band: "
            f"{format_money(band.get('low'))} / {format_money(band.get('median'))} / {format_money(band.get('high'))}."
        )
    return summary


def render_template(report_data: dict[str, Any]) -> str:
    template = TEMPLATE_PATH.read_text(encoding="utf-8")
    payload = json.dumps(report_data, ensure_ascii=False, indent=2)
    escaped_payload = payload.replace("</", "<\\/")
    replacements = {
        "{{REPORT_JSON}}": escaped_payload,
        "{{REPORT_TITLE}}": html.escape(report_title(report_data)),
        "{{GENERATED_AT}}": html.escape(str(report_data.get("generated_at", ""))),
    }
    for placeholder, value in replacements.items():
        template = template.replace(placeholder, value)
    return template


def report_title(report_data: dict[str, Any]) -> str:
    company = report_data.get("company", {})
    ticker = company.get("ticker", "")
    name = company.get("name", "")
    return " / ".join(part for part in (ticker, name) if part) or "Stock Narrative Research"


def load_sections(workdir: Path, errors: list[str]) -> dict[str, Any]:
    sections: dict[str, Any] = {}
    for key, filename in SECTION_FILES.items():
        path = workdir / "sections" / filename
        try:
            sections[key] = read_json(path)
        except Exception as exc:
            errors.append(f"Could not read sections/{filename}: {exc}")
    return sections


def ensure_run(workdir: Path) -> None:
    if not (workdir / "manifest.json").exists():
        raise SystemExit(f"Not a report run directory: {workdir}")
    for dirname in ("raw", "sections", "generated"):
        if not (workdir / dirname).exists():
            raise SystemExit(f"Missing {dirname}/ in report run directory: {workdir}")


def resolve_workdir(ticker: str, workdir_arg: str | None) -> Path:
    if workdir_arg:
        path = Path(workdir_arg)
        return path if path.is_absolute() else WORKSPACE_DIR / path
    candidates = sorted(OUTPUTS_DIR.glob(f"{ticker}-*"), key=lambda path: path.stat().st_mtime, reverse=True)
    if not candidates:
        raise SystemExit(f"No run directory found for {ticker}. Run init first or pass --workdir.")
    return candidates[0]


def update_manifest(workdir: Path, status_key: str, value: bool = True) -> None:
    manifest_path = workdir / "manifest.json"
    manifest = read_json(manifest_path)
    manifest.setdefault("status", {})[status_key] = value
    manifest["updated_at"] = now_iso()
    write_json(manifest_path, manifest)


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False, sort_keys=True) + "\n", encoding="utf-8")


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def timestamp_slug() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")


def is_blank(value: Any) -> bool:
    if value is None:
        return True
    if value == "":
        return True
    if isinstance(value, list):
        return len(value) == 0 or all(is_blank(item) for item in value)
    if isinstance(value, dict):
        return not any(not is_blank(item) for item in value.values())
    return False


def has_filled_item(value: Any, required_keys: list[str]) -> bool:
    if not isinstance(value, list):
        return False
    for item in value:
        if not isinstance(item, dict):
            continue
        if all(not is_blank(item.get(key)) for key in required_keys):
            return True
    return False


def is_number(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def format_money(value: Any) -> str:
    if not is_number(value):
        return "n/a"
    return f"${value:,.0f}"


def format_optional_money(value: Any) -> str | None:
    if not is_number(value):
        return None
    if abs(value) >= 1_000_000_000:
        return f"${value / 1_000_000_000:,.1f}B"
    if abs(value) >= 1_000_000:
        return f"${value / 1_000_000:,.1f}M"
    return f"${value:,.2f}"


def format_optional_percent(value: Any) -> str | None:
    if not is_number(value):
        return None
    return f"{value * 100:.1f}%"


def slug_key(value: str) -> str:
    return "".join(char.lower() if char.isalnum() else "_" for char in value).strip("_")


if __name__ == "__main__":
    sys.exit(main())
