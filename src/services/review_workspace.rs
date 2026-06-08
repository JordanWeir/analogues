pub fn workspace_schema_hint() -> &'static str {
    r#"Workspace SQLite tables available via workspace_sql:
- canonical_metric_definitions(canonical_key, metric_key, metric_label, statement_type, unit_hint, display_order)
- concept_catalog_entries(taxonomy, concept_name, label, description, unit, fact_count, earliest_period_end, latest_period_end, latest_filed_at, min_value, max_value, dominant_period_shape, series_usability, plot_readiness, narrative_tags)
- sec_raw_facts(taxonomy, concept_name, label, description, unit, form, period_start, period_end, filed_at, fiscal_year, fiscal_period, accession, frame, metric_value, fetched_at)
- raw_fact_metric_catalog view: grouped concept inventory from sec_raw_facts

Start by selecting all rows from canonical_metric_definitions ordered by display_order.
For each canonical metric, investigate concept_catalog_entries and sec_raw_facts to choose the best direct SEC XBRL concept.
Query latest metric_value and period_end for your selected concept before validating online."#
}
