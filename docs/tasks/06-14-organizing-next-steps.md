# Organizing next steps

We have a lot of components in a 'kind of works' state.

Our overall end to end system right now is taking 30 minutes to run.

We make a lot of tool calls from a lot of agents, and don't have a clean way of understanding what specific worker actually did.

Good Worker Observability:
- I can see if it went down failed paths, and consider prompt changes / golden path corrections
- I can see if there are tools it's using poorly where the schema could be improved
- *Maybe* visibility into it's thinking/reasoning before calling tools. This may be surprisingly unneeded though



Next Steps:
- Clean Fixtures of each stage.  
    - Task to generate fixtures? Run lane against fixture?
    - I kind of want a terminal app for managing loco tasks, passing in paths feels pretty bad
- Clean Runs of Phases Independently
- Tracking Tool Calls + Results associated with each run
- (DONE) Add a Retry when a scenario builder detail worker fails
- Generate an HTML file based on Scenarios

