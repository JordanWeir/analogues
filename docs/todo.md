# Fundamentals Extraction

- The XBRL parsing based on keywords is unreliable
- We should extract all time series we get into the DB in a raw data table
- We should query to get a list of metric names
- A specific agent should look at the metric name list and specifically discover the "core" metric names we care about, and build a "canonical view" table
- If there are metrics we want to *always* show in the UI, we should have a table that specifically translates those.
- Core Labels
    - Revenue
    - Net Income
    - Share Count / Float
    - Key Balance Sheet Items
- Later analysis during the narrative building stage should review the full metrics list again and create a list of "supporting metrics" for that scenario
- UI should render historical timeline data based on:
    - "Canonical View" of core metrics. eg: if we want net income, we may find in the raw data the metric is "GlobalNetIncome", so we'd flag that as the canonical Net Income metric
    - "Supporting Metrics" are scenario-specific metrics the LLM chooses to include in the analysis, and are custom to that analysis
- 