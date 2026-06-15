# Report UI Quality

We've now synced the generate report functionality again, but we've lost a couple big things in transition.

1. We only project P/E and P/S multiples in the terminal period; this means we don't get a nice chart.  The scenario agent needs to do a full projection for each point in the timeline.
2. We don't include current price; this also hurts our nicer chart, and led to regressions.
3. We're linking a lot of raw data in ways that are pretty awkward; This should be resolved once we finish the content manager lane
4. Missing High Value Derived Time Series: Earnings per Share, Revenue per Share, Quarter Price HLOC, Min/Max P/E per Quarter, Min/Max Price/Revenue per Quarter.  Elevating these as critical derived metrics in highly visible time series should help ground financial explorations, experiments, and future projections.
5. Not all time series have the same terminal endpoint; blueprint should align the timespan before scenarios are built
6. We are projecting a *quarterly* EPS of $2.96 in the report, but then we're applying a P/E multiple to that quarterly to get a range.  We should really calculate the 12 TTM EPS/Revenue numbers and apply the multiple to that instead.
7. Double check the scenario build process; possibly a better general approach would be to submit one period at a time, rolling forward, maybe after doing forward projections in SQL.
8. For the Projections Table that we show in the UI, we should also show maybe 3 years of historical past data, with a visual indication of past vs projected.