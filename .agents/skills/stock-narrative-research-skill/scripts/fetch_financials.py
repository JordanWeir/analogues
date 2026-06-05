#!/usr/bin/env python3
"""Fetch a public financial snapshot for a ticker.

The helper prefers primary SEC Company Facts for fundamentals, uses yfinance
when available for market-data fields, and falls back to Yahoo's public chart
endpoint for a limited quote snapshot.
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


SEC_USER_AGENT = "stock-narrative-research/0.1 research@example.local"
SEC_TICKERS_URL = "https://www.sec.gov/files/company_tickers.json"
SEC_COMPANYFACTS_URL = "https://data.sec.gov/api/xbrl/companyfacts/CIK{cik}.json"

FUNDAMENTAL_KEYS = {
    "company_name",
    "revenue_ttm",
    "net_income_ttm",
    "gross_profit_ttm",
    "operating_income_ttm",
    "gross_margin",
    "operating_margin",
    "net_margin",
    "eps_ttm",
    "shares_outstanding",
    "cash",
    "total_debt",
    "fundamental_period_end",
    "fundamental_source",
}


def main() -> int:
    parser = argparse.ArgumentParser(description="Fetch a basic financial snapshot for a ticker.")
    parser.add_argument("ticker", help="Ticker symbol, for example SMCI")
    parser.add_argument("--output", "-o", help="Write JSON to this path instead of stdout")
    args = parser.parse_args()

    snapshot = fetch_snapshot(args.ticker.upper())
    write_json(snapshot, args.output)
    return 0


def fetch_snapshot(ticker: str) -> dict[str, Any]:
    snapshot = base_snapshot(ticker)

    try:
        yfinance_snapshot = fetch_with_yfinance(ticker)
    except Exception as exc:  # pragma: no cover - depends on optional package/network
        snapshot["source_notes"].append(f"yfinance unavailable or failed: {exc}")
        yfinance_snapshot = {}

    merge_snapshot(snapshot, yfinance_snapshot, overwrite=False)

    if snapshot.get("current_price") is None:
        try:
            chart_snapshot = fetch_with_yahoo_chart(ticker)
            merge_snapshot(snapshot, chart_snapshot, overwrite=False)
        except Exception as exc:  # pragma: no cover - depends on network
            snapshot["source_notes"].append(f"Yahoo chart fallback failed: {exc}")

    try:
        sec_snapshot = fetch_with_sec_companyfacts(ticker)
        merge_snapshot(snapshot, sec_snapshot, overwrite=True, keys=FUNDAMENTAL_KEYS)
    except Exception as exc:  # pragma: no cover - depends on SEC availability/network
        snapshot["source_notes"].append(f"SEC Company Facts unavailable or failed: {exc}")

    compute_derived_metrics(snapshot)
    mark_gaps(snapshot)
    return snapshot


def base_snapshot(ticker: str) -> dict[str, Any]:
    return {
        "ticker": ticker,
        "fetched_at": datetime.now(timezone.utc).isoformat(),
        "source": "SEC Company Facts for fundamentals; yfinance/Yahoo for market data",
        "data_sources": [],
        "currency": None,
        "company_name": None,
        "current_price": None,
        "market_cap": None,
        "shares_outstanding": None,
        "revenue_ttm": None,
        "net_income_ttm": None,
        "gross_profit_ttm": None,
        "operating_income_ttm": None,
        "gross_margin": None,
        "operating_margin": None,
        "net_margin": None,
        "eps_ttm": None,
        "trailing_pe": None,
        "price_to_sales_ttm": None,
        "enterprise_to_revenue": None,
        "enterprise_to_ebitda": None,
        "cash": None,
        "total_debt": None,
        "fundamental_period_end": None,
        "fundamental_source": None,
        "source_notes": [],
        "gaps": [],
    }


def fetch_with_yfinance(ticker: str) -> dict[str, Any]:
    import yfinance as yf  # type: ignore[import-not-found]

    equity = yf.Ticker(ticker)
    info = equity.get_info()
    fast_info = getattr(equity, "fast_info", {}) or {}

    current_price = pick_number(
        fast_info.get("last_price"),
        info.get("currentPrice"),
        info.get("regularMarketPrice"),
        info.get("previousClose"),
    )
    shares = pick_number(info.get("sharesOutstanding"), info.get("impliedSharesOutstanding"))
    revenue_per_share = pick_number(info.get("revenuePerShare"))
    revenue_from_per_share = revenue_per_share * shares if revenue_per_share is not None and shares is not None else None
    revenue = pick_number(info.get("totalRevenue"), revenue_from_per_share)

    return {
        "currency": info.get("currency"),
        "company_name": info.get("longName") or info.get("shortName"),
        "current_price": current_price,
        "market_cap": pick_number(info.get("marketCap"), fast_info.get("market_cap")),
        "shares_outstanding": shares,
        "revenue_ttm": revenue,
        "gross_margin": pick_number(info.get("grossMargins")),
        "operating_margin": pick_number(info.get("operatingMargins")),
        "net_margin": pick_number(info.get("profitMargins")),
        "eps_ttm": pick_number(info.get("trailingEps")),
        "trailing_pe": pick_number(info.get("trailingPE")),
        "price_to_sales_ttm": pick_number(info.get("priceToSalesTrailing12Months")),
        "enterprise_to_revenue": pick_number(info.get("enterpriseToRevenue")),
        "enterprise_to_ebitda": pick_number(info.get("enterpriseToEbitda")),
        "cash": pick_number(info.get("totalCash")),
        "total_debt": pick_number(info.get("totalDebt")),
        "data_sources": ["yfinance"],
        "source_notes": ["Fetched via yfinance. Verify against primary filings before relying on figures."],
    }


def fetch_with_yahoo_chart(ticker: str) -> dict[str, Any]:
    url = f"https://query1.finance.yahoo.com/v8/finance/chart/{ticker}?range=1d&interval=1d"
    request = Request(url, headers={"User-Agent": "Mozilla/5.0"})
    try:
        with urlopen(request, timeout=15) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except URLError as exc:
        raise RuntimeError(str(exc)) from exc

    result = (payload.get("chart", {}).get("result") or [{}])[0]
    meta = result.get("meta", {})
    return {
        "currency": meta.get("currency"),
        "company_name": meta.get("shortName") or meta.get("longName"),
        "current_price": pick_number(meta.get("regularMarketPrice"), meta.get("previousClose")),
        "data_sources": ["Yahoo chart endpoint"],
        "source_notes": ["Fetched limited price metadata from Yahoo chart endpoint. Fundamental fields require yfinance or manual input."],
    }


def fetch_with_sec_companyfacts(ticker: str) -> dict[str, Any]:
    company = lookup_sec_company(ticker)
    cik = str(company["cik_str"]).zfill(10)
    payload = fetch_json(SEC_COMPANYFACTS_URL.format(cik=cik), sec_headers())
    us_gaap = payload.get("facts", {}).get("us-gaap", {})

    revenue, revenue_note = ttm_value(
        us_gaap,
        [
            "RevenueFromContractWithCustomerExcludingAssessedTax",
            "Revenues",
            "SalesRevenueNet",
        ],
    )
    net_income, net_income_note = ttm_value(us_gaap, ["NetIncomeLoss"])
    gross_profit, gross_profit_note = ttm_value(us_gaap, ["GrossProfit"])
    operating_income, operating_income_note = ttm_value(us_gaap, ["OperatingIncomeLoss"])

    shares = latest_value(
        us_gaap,
        [
            "WeightedAverageNumberOfDilutedSharesOutstanding",
            "CommonStocksIncludingAdditionalPaidInCapitalMember",
            "CommonStockSharesOutstanding",
        ],
        unit_hint="shares",
    )
    eps = latest_value(us_gaap, ["EarningsPerShareDiluted"], unit_hint="USD/shares")
    cash = latest_value(
        us_gaap,
        [
            "CashAndCashEquivalentsAtCarryingValue",
            "CashCashEquivalentsRestrictedCashAndRestrictedCashEquivalents",
        ],
        unit_hint="USD",
    )
    debt = total_latest_values(
        us_gaap,
        [
            ["DebtCurrent", "LongTermDebtAndFinanceLeaseObligationsCurrent"],
            ["LongTermDebtAndFinanceLeaseObligationsNoncurrent", "LongTermDebtAndCapitalLeaseObligations"],
        ],
        unit_hint="USD",
    )

    period_end = latest_end(revenue_note, net_income_note, gross_profit_note, operating_income_note)
    source_notes = [
        "Fetched fundamentals from SEC Company Facts. TTM values prefer latest annual + current interim - prior interim; otherwise they fall back to four quarters or the latest annual period.",
    ]
    for note in (revenue_note, net_income_note, gross_profit_note, operating_income_note):
        if note and note not in source_notes:
            source_notes.append(note)

    return {
        "company_name": company.get("title"),
        "revenue_ttm": revenue,
        "net_income_ttm": net_income,
        "gross_profit_ttm": gross_profit,
        "operating_income_ttm": operating_income,
        "gross_margin": ratio(gross_profit, revenue),
        "operating_margin": ratio(operating_income, revenue),
        "net_margin": ratio(net_income, revenue),
        "eps_ttm": ratio(net_income, shares) if net_income is not None and shares is not None else eps,
        "shares_outstanding": shares,
        "cash": cash,
        "total_debt": debt,
        "fundamental_period_end": period_end,
        "fundamental_source": "SEC Company Facts",
        "data_sources": ["SEC Company Facts"],
        "source_notes": source_notes,
    }


def lookup_sec_company(ticker: str) -> dict[str, Any]:
    payload = fetch_json(SEC_TICKERS_URL, sec_headers())
    ticker_upper = ticker.upper()
    for company in payload.values():
        if str(company.get("ticker", "")).upper() == ticker_upper:
            return company
    raise RuntimeError(f"Ticker {ticker} was not found in SEC company_tickers.json")


def fetch_json(url: str, headers: dict[str, str] | None = None) -> dict[str, Any]:
    request = Request(url, headers=headers or {"User-Agent": "Mozilla/5.0"})
    try:
        with urlopen(request, timeout=20) as response:
            return json.loads(response.read().decode("utf-8"))
    except HTTPError as exc:
        raise RuntimeError(f"{exc.code} {exc.reason}") from exc
    except URLError as exc:
        raise RuntimeError(str(exc)) from exc


def sec_headers() -> dict[str, str]:
    return {"User-Agent": SEC_USER_AGENT}


def merge_snapshot(
    snapshot: dict[str, Any],
    update: dict[str, Any],
    *,
    overwrite: bool,
    keys: set[str] | None = None,
) -> None:
    for key, value in update.items():
        if value is None:
            continue
        if key == "source_notes":
            for note in value:
                if note not in snapshot["source_notes"]:
                    snapshot["source_notes"].append(note)
            continue
        if key == "data_sources":
            for source in value:
                if source not in snapshot["data_sources"]:
                    snapshot["data_sources"].append(source)
            continue
        if keys is not None and key not in keys:
            continue
        if overwrite or snapshot.get(key) is None:
            snapshot[key] = value


def compute_derived_metrics(snapshot: dict[str, Any]) -> None:
    price = pick_number(snapshot.get("current_price"))
    shares = pick_number(snapshot.get("shares_outstanding"))
    revenue = pick_number(snapshot.get("revenue_ttm"))
    net_income = pick_number(snapshot.get("net_income_ttm"))
    gross_profit = pick_number(snapshot.get("gross_profit_ttm"))
    operating_income = pick_number(snapshot.get("operating_income_ttm"))
    eps = pick_number(snapshot.get("eps_ttm"))

    if snapshot.get("market_cap") is None and price is not None and shares is not None:
        snapshot["market_cap"] = price * shares
    if snapshot.get("gross_margin") is None:
        snapshot["gross_margin"] = ratio(gross_profit, revenue)
    if snapshot.get("operating_margin") is None:
        snapshot["operating_margin"] = ratio(operating_income, revenue)
    if snapshot.get("net_margin") is None:
        snapshot["net_margin"] = ratio(net_income, revenue)
    if snapshot.get("eps_ttm") is None and net_income is not None and shares is not None:
        snapshot["eps_ttm"] = net_income / shares
    if snapshot.get("trailing_pe") is None and price is not None and eps not in (None, 0):
        snapshot["trailing_pe"] = price / eps
    if snapshot.get("price_to_sales_ttm") is None and snapshot.get("market_cap") is not None and revenue not in (None, 0):
        snapshot["price_to_sales_ttm"] = snapshot["market_cap"] / revenue


def ttm_value(us_gaap: dict[str, Any], concepts: list[str]) -> tuple[float | None, str | None]:
    facts = statement_facts(us_gaap, concepts)
    if not facts:
        return None, None

    annual = latest_duration_fact(facts, forms={"10-K"}, min_days=250, max_days=380)
    interim = latest_duration_fact(
        facts,
        forms={"10-Q"},
        min_days=60,
        max_days=300,
        end_after=annual.get("end") if annual else None,
    )
    if annual and interim:
        prior = comparable_prior_interim(facts, interim)
        if prior:
            value = float(annual["val"]) + float(interim["val"]) - float(prior["val"])
            note = f"{concept_name(interim)} TTM bridged through {interim.get('end')} from annual plus current/prior interim periods."
            return value, note

    quarters = latest_quarter_facts(facts)
    if len(quarters) >= 4:
        value = sum(float(fact["val"]) for fact in quarters[:4])
        note = f"{concept_name(quarters[0])} TTM summed from the latest four quarterly facts through {quarters[0].get('end')}."
        return value, note

    if annual:
        note = f"{concept_name(annual)} used latest annual value through {annual.get('end')} because TTM bridge was unavailable."
        return float(annual["val"]), note
    return None, None


def statement_facts(us_gaap: dict[str, Any], concepts: list[str]) -> list[dict[str, Any]]:
    facts: list[dict[str, Any]] = []
    for concept in concepts:
        concept_payload = us_gaap.get(concept, {})
        for unit, values in concept_payload.get("units", {}).items():
            for value in values:
                if value.get("val") is None or not value.get("start") or not value.get("end"):
                    continue
                fact = dict(value)
                fact["_concept"] = concept
                fact["_unit"] = unit
                facts.append(fact)
    return facts


def latest_value(us_gaap: dict[str, Any], concepts: list[str], *, unit_hint: str) -> float | None:
    facts = []
    for concept in concepts:
        concept_payload = us_gaap.get(concept, {})
        for unit, values in concept_payload.get("units", {}).items():
            if unit_hint.lower() not in unit.lower():
                continue
            for value in values:
                if value.get("val") is None or not value.get("end"):
                    continue
                facts.append(value)
    if not facts:
        return None
    fact = sorted(facts, key=lambda item: (item.get("end", ""), item.get("filed", "")), reverse=True)[0]
    return pick_number(fact.get("val"))


def total_latest_values(us_gaap: dict[str, Any], concept_groups: list[list[str]], *, unit_hint: str) -> float | None:
    values = [latest_value(us_gaap, concepts, unit_hint=unit_hint) for concepts in concept_groups]
    numeric_values = [value for value in values if value is not None]
    return sum(numeric_values) if numeric_values else None


def latest_duration_fact(
    facts: list[dict[str, Any]],
    *,
    forms: set[str],
    min_days: int,
    max_days: int,
    end_after: str | None = None,
) -> dict[str, Any] | None:
    candidates = [
        fact
        for fact in facts
        if fact.get("form") in forms
        and min_days <= duration_days(fact) <= max_days
        and (end_after is None or fact.get("end", "") > end_after)
    ]
    if not candidates:
        return None
    return sorted(candidates, key=lambda item: (item.get("end", ""), item.get("filed", "")), reverse=True)[0]


def comparable_prior_interim(facts: list[dict[str, Any]], interim: dict[str, Any]) -> dict[str, Any] | None:
    fy = interim.get("fy")
    fp = interim.get("fp")
    if not isinstance(fy, int) or not fp:
        return None
    candidates = [
        fact
        for fact in facts
        if fact.get("form") == "10-Q"
        and fact.get("fy") == fy - 1
        and fact.get("fp") == fp
        and fact.get("_concept") == interim.get("_concept")
    ]
    if not candidates:
        return None
    return sorted(candidates, key=lambda item: (item.get("end", ""), item.get("filed", "")), reverse=True)[0]


def latest_quarter_facts(facts: list[dict[str, Any]]) -> list[dict[str, Any]]:
    quarters = [
        fact
        for fact in facts
        if fact.get("form") in {"10-Q", "10-K"}
        and 60 <= duration_days(fact) <= 120
        and fact.get("val") is not None
    ]
    unique_by_end: dict[str, dict[str, Any]] = {}
    for fact in sorted(quarters, key=lambda item: (item.get("end", ""), item.get("filed", "")), reverse=True):
        unique_by_end.setdefault(str(fact.get("end")), fact)
    return list(unique_by_end.values())


def duration_days(fact: dict[str, Any]) -> int:
    try:
        start = datetime.fromisoformat(str(fact["start"]))
        end = datetime.fromisoformat(str(fact["end"]))
    except (KeyError, ValueError):
        return 0
    return (end - start).days


def concept_name(fact: dict[str, Any] | None) -> str:
    if not fact:
        return "SEC fact"
    return str(fact.get("_concept") or "SEC fact")


def latest_end(*notes: str | None) -> str | None:
    ends = []
    for note in notes:
        if not note:
            continue
        marker = " through "
        if marker in note:
            ends.append(note.split(marker, 1)[1].split(" ", 1)[0].rstrip("."))
    return max(ends) if ends else None


def ratio(numerator: float | None, denominator: float | None) -> float | None:
    if numerator is None or denominator in (None, 0):
        return None
    return numerator / denominator


def mark_gaps(snapshot: dict[str, Any]) -> None:
    required = {
        "current_price": "current share price",
        "market_cap": "market cap",
        "shares_outstanding": "share count",
        "revenue_ttm": "revenue",
        "net_margin": "net margin",
        "eps_ttm": "EPS",
    }
    snapshot["gaps"] = [label for key, label in required.items() if snapshot.get(key) is None]


def pick_number(*values: Any) -> float | None:
    for value in values:
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            return float(value)
    return None


def write_json(payload: dict[str, Any], output_path: str | None) -> None:
    text = json.dumps(payload, indent=2, sort_keys=True)
    if output_path:
        path = Path(output_path)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text + "\n", encoding="utf-8")
    else:
        print(text)


if __name__ == "__main__":
    sys.exit(main())
