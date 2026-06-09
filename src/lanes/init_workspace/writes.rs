/// Tables written by the `init_workspace` lane (phase 1 ingest).
pub const TABLES_WRITTEN: &[&str] = &[
    "stock_info",
    "sec_raw_facts",
    "fundamentals",
    "fundamental_observations",
    "run_metadata",
    "data_gaps",
];
