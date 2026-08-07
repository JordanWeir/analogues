# More Problems

After we switched to inserting one period at a time...
- Seeing a lot more "failed to generate scenarios"
    - `Error: Message("scenario_generation failed: not every scenario has persisted quarterly periods")`
    - This is likely a number of steps problem, when it has a couple errors in the period logic.
- UI isn't showing historical P/E and P/S ranges
- UI Chart is showing two sets of overlapping X Axis; This might be a normalization issue with the historical vs projected numbers
- UI Chart doesn't show the current point on the chart; The X axis should be a smooth evenly spaced timeline, and todays date and price should be correctly fit onto that timeline
- Past quarters on that timeline should show High / Low bands with a midpoint, similar to the style used in our projections
- Tables should show P/S (TTM) and P/E (TTM)
- Need to investigate the output numbers we're getting for: Rev, Margin, Net Income, Share Counts, EPS, Rev PS, and multiples.  It seems like that we should be deriving more of that then we currently are, and it's not clear those are staying synced as a real model.  Maybe each period should get { Rev, Margin, Sharecount, MultipleRanges } and Net Income / EPS should be derived?
- There's cases where we're assigning the same upper / mid / lower bands to the multiples.  This leads to weird results; we should probably guide to around 20% diffs.
- Table growth rate conventions feel a little weird; we should check what is commonly done online and see if there's a convention to match
- Scenario Period Multiples seem sketch; we should make sure we're passing historical multiples in as context so it's clear what 'reasonable' numbers look like for that stock.