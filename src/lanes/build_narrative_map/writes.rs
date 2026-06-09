/// Tables read by the `build_narrative_map` lane.
pub const TABLES_READ: &[&str] = &[
    "run_metadata",
    "stock_info",
    "fundamentals",
    "concept_catalog_entries",
    "canonical_metric_mappings",
];

/// Tables written by the `build_narrative_map` lane.
pub const TABLES_WRITTEN: &[&str] = &[
    "sources",
    "claims",
    "narrative_map",
    "narrative_map_items",
    "sections",
    "data_gaps",
    "stock_info",
];
