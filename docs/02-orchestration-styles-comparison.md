# Orchestration Styles Comparison

One of the main questions with this pipeline is how we actually want to orchestrate and manage the research.

The "Skills" based method demonstrated that an LLM with a skills file outlining what to do, and a handful of scripting artifacts, is sufficient to do an "ok" job, although with a pretty high cost.

Productization requires:
- Reducing the cost
    - Maybe weaker models + stronger guard rails
    - Better context management so we aren't overpaying for context history
- Better Data Accuracy
    - This is so far a scripting problem, not a model intelligence issue
    - Initially, we wondered if this was a build/buy question and we should be using things like AlphaVantage as a data source
    - Instead we realized this can be a serious edge; 
        - ORCL for example has 512 concepts in the SEC Facts API.  
        - Detailed time series of key financial data lets us run deeper financial model simulations to identify hidden cruxes and relationships
        - Using agents to identify the Canonical, High Value, and Crux related patterns in this data gives us a strong advantage on the naive workflow
        - This doesn't mean it couldn't be handled by a Skill still; if a Skill could spin up sub agents, it could have sub agents work on many of these tasks
        - This expanded scope DOES make cost optimization more important; shifts us towards models on Open Router instead of Anthropic / OpenAI
- A sturdy architecture foundation
    - The Skills method feels architecturally fragile
        - Relies heavily on agents calling scripts at the right time, and things like parallelization of agents have limits
    - We were able to port from the agent python files to loco tasks, although first draft of loco tasks was super ugly, and 'giant one file' patterned
    - Loco tasks have access to the rest of the workspace; there is no reason in principle we can't design a system properly, and still have agents call loco tasks to execute
- Cost Observability
    - The Skills method makes it really hard to understand where your token costs are coming from.
        - Cost signal is purely the Cursor websites "Task cost $1.15"
    - An application-based agent system allows you to look at the actual token costs for each step, aggregate them appropriately, and make better strategic choices around model-per-task, task breakdown, and context window management
    - With the "refresh" pattern unknown, it's critical to understand cost implications

## Three Levels of Orchestration Discussed so far

1. Skills
2. Linear phase system
3. Blackboard Research system

These are not three totally separate products. They are best understood as three levels of control-plane sophistication over a shared research workspace.

The durable piece should be the run workspace: SQLite tables, source custody, concept catalogs, insights, scenarios, calculations, and rendered artifacts. The swappable piece should be the orchestration strategy that decides which worker runs next and what context it receives.

## Level 1: Skills-Based Orchestration

The skills approach uses a written agent skill as the primary control plane. The skill tells a capable model what to do, which scripts to call, what tables to inspect, and how to assemble the final research artifact.

This is the fastest way to prove that the workflow can work at all. It leans heavily on the model's general planning ability and requires relatively little application scaffolding. The agent can notice weird facts, improvise around missing data, and adjust the work plan without the application needing to model every state transition.

### Character

- Prompt-led orchestration.
- Minimal durable process state beyond whatever the agent writes into the workspace.
- High flexibility and low upfront engineering cost.
- High dependence on one model staying on-task over a long run.
- Good for proof-of-concept research and discovering the workflow shape.
- Weak for repeatability, observability, bounded cost, and product reliability.

### Strengths

- Fastest to build and revise.
- Excellent for exploratory work where we do not yet know the right decomposition.
- Lets the agent discover missing steps organically.
- Useful for generating examples that later become product requirements.

### Weaknesses

- Cost is hard to attribute to specific research activities.
- Context can grow without clear boundaries.
- Agents may call tools in inconsistent order or skip important validation.
- Parallelization is awkward and dependent on the agent environment.
- Intermediate reasoning may live in chat rather than durable tables.
- Hard to test because the "program" is partly a prose instruction file and partly model behavior.

### Best Use

Use skills to keep exploring the outer boundary of what the product should do, especially for new report types, new research questions, and examples that are not yet stable enough to productize.

Do not rely on skills as the main production architecture once the desired work pattern is understood.

## Level 2: Linear Phase System

The linear phase system turns the research workflow into a deterministic sequence of application-managed stages. Each stage has a clear input/output contract and writes durable state into the SQLite workspace.

The key improvement over skills is that the application owns the run shape. Agents can still perform judgment-heavy work inside a phase, but the phase runner controls ordering, context loading, validation, retries, model choice, and cost tracking.

This is a natural first product architecture because it converts the discovered skill workflow into something observable and testable without requiring the full complexity of a blackboard runtime.

### Character

- Application-led orchestration.
- Phase boundaries are input/output contracts, not necessarily deterministic-vs-LLM boundaries.
- SQLite acts as a pseudo-blackboard.
- Workers can be model-backed, deterministic, or hybrid.
- Easier to reason about than a fully dynamic research loop.
- Risk of forced linearity if late insights cannot reopen earlier work.

### Strengths

- Much better cost observability than skills.
- Easier to test one phase at a time.
- Clear handoffs between raw data, canonical data, source pack, insights, scenarios, and artifacts.
- Straightforward to checkpoint and resume.
- Enables weaker/cheaper models for scoped tasks.
- Allows deterministic validation after each major handoff.
- Fits a SQLite-first calculation strategy well: deterministic workers can run both historical investigations and lightweight forward projections directly against the workspace.
- Can cheaply support weaker models if we provide a small exemplar library, such as 5 sample "historical investigation" queries and 5 sample "forward projection" queries they can adapt.

### Weaknesses

- Can make research feel more linear than it really is.
- Late discoveries may be awkward unless the system has explicit backflow signals.
- Phase boundaries can become too numerous if every worker type is treated as a separate phase.
- Agents may still need access to earlier context, so context management does not disappear.
- If implemented naively, each phase can become bespoke SQL plus bespoke prompts, making later blackboard migration harder.

### Best Use

Use this as the likely first production implementation.

The important design constraint is to keep the phase runner disposable. The durable pieces should be:

- Workspace schema.
- Typed store APIs.
- Reusable workers.
- Validators.
- Insight, signal, and obligation ledgers.

If those boundaries are respected, a linear runner can be replaced or supplemented later by a blackboard runner without rewriting all research logic.

## Level 3: Blackboard Research System

The blackboard research system makes the shared workspace the center of the architecture. Workers do not run because a fixed phase says it is their turn. They run because the board contains open signals, unresolved obligations, contradictions, gaps, or immature outputs.

In this model, source research, narrative mapping, SEC concept triage, financial experiments, analogue research, crux synthesis, and scenario drafting are worker lanes inside an iterative loop. The orchestrator repeatedly inspects board state, dispatches workers, reviews new entries, opens or closes signals, and decides whether the board is mature enough to compile scenarios and render a report.

This better matches the shape of real research. A financial experiment can discover a new crux. A new crux can trigger more web research. New research can update the narrative map. Updated narratives can invalidate scenario assumptions. The system converges when critical obligations are satisfied, waived, or carried into the report as limitations.

### Character

- Board-led orchestration.
- Dynamic dispatch based on signals, obligations, gaps, and contradictions.
- Workers return structured entries rather than final prose.
- Late discoveries can trigger targeted backtracking.
- More robust for complex research, but more expensive to design and test.
- Requires a stronger data model for epistemic status, provenance, relations, and supersession.

### Strengths

- Handles non-linear research naturally.
- Makes late insights first-class instead of disruptive.
- Supports parallel worker dispatch when multiple signals are open.
- Produces better auditability if entries, signals, obligations, and worker runs are persisted.
- Can converge based on quality gates rather than fixed phase completion.
- Better fit for refresh workflows where new information invalidates only part of the report.
- Still benefits from the same SQLite-first calculation surface; workers can open targeted signals like "run the capex funding projection query" or "rerun the backlog conversion investigation" without needing a separate math runtime.

### Weaknesses

- More architecture upfront.
- Harder to debug than a linear runner because run paths are dynamic.
- Requires good prioritization or it can chase too many open questions.
- Needs clear convergence criteria to avoid endless research.
- Cost controls must be explicit: max iterations, max workers, model budgets, and signal priority thresholds.
- Quality gates need to be strong enough to prevent low-quality entries from polluting the board.

### Best Use

Use this when the linear pipeline starts showing strain:

- Late crux discovery frequently forces manual rework.
- Reports vary enough that fixed phases feel unnatural.
- Multiple workers could usefully run in parallel.
- Refreshes need partial invalidation rather than full regeneration.
- The team wants detailed observability into research state, not only final artifacts.

## Practical Migration Path

The safest path is not to choose between linear and blackboard immediately. Build the first product as a linear runner over blackboard-compatible ledgers.

That means the initial implementation can run in a simple order:

1. Initialize workspace.
2. Build data and concept catalogs.
3. Build source pack and narrative map.
4. Generate insights and financial experiments.
5. Compile scenarios.
6. Calculate and render.

But each step should still write structured entries, signals, obligations, worker run records, and validation results.

Later, a blackboard runner can use the same underlying workers and tables with a different control loop:

1. Inspect open signals and obligations.
2. Dispatch the best worker or workers.
3. Review and persist entries.
4. Open, close, or reprioritize signals.
5. Check scenario and report readiness.
6. Repeat until convergence or budget limit.

## Design Implication

The main architectural decision is to keep orchestration separate from research capability.

Workers should not know whether they are being called by a skill, a linear phase, or a blackboard loop. They should receive scoped context and return structured outputs:

- Entries or insights.
- Source-backed claims.
- Supporting metric selections.
- Signals opened or closed.
- Scenario assumptions or revisions.
- Quality flags.
- Cost and usage metadata.

If that boundary holds, the orchestration style becomes swappable. If it does not, the first linear implementation will hard-code too many assumptions and make the blackboard experiment feel like a rewrite.

