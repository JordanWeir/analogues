# Product Boundary
Is the first system map for v0.1 only, or should it show the eventual v1 shape with staged parts marked?
- Show the full eventual shape, with staged parts marked

Is the initial implementation definitely Rust/Loco, or should the system map stay framework-neutral?
- Definitely Rust/Loco.  Unclear if we'll do Datastar / HTMX / React on frontend.

Should the map optimize for one developer moving fast, or for multiple LLM agents working in parallel?
- Optimize for multiple agents working in parallel
- Also optimize for long term maintainability via strong division of responsibility + high testabilty

# MVP Scope
For the first slice, should the app support only curated AI infrastructure tickers, or should ingestion/search be generalized from day one?
- Lets focus on curated only
- In the longer run, I think we actually want to lean more on "once a day", scheduled automations or breaking-new event-triggered automations, and not user-triggered automations.
- User's aren't special snowflakes; the stock market is the same for everyone

What is the minimum “magic moment” page: current narrative only, or current narrative plus historical analogues?
- Current narrative + historical analogues, but the early versions of historical analogues can be rough

Are historical episodes manually curated at first, semi-automated, or generated from retrieved sources?
- generated from retrieved sources
- we should have *something* here, and the module interface should make sense, but we don't need a 100% perfect solution
- This only needs to be correct enough that people can see what we're going for in the beginning

# Data And Sources
What source types are in-scope for v0.1: filings, transcripts, press releases, financial news, blogs/newsletters, price/returns data?
- Lets choose a small handful of blog + news websites, and see if we can search that very small group for sufficient data
- It's fine if we only go back to maybe 2010 or 2015 if that simplifies the short term
- We care much more about what people and analysts were generally discussing in public, vs things like specific filings

Do you already have preferred vendors/APIs for market data, transcripts, news, or embeddings?
- For market data/pricing, lets go data bento
- For everything else, pretty flexible.  News and strategy blog posts could ideally be things like seeking alpha, zacks, or WSJ.  Open to other options here though.  Even forum discussions would be fine, although not sure how/where we would get those.

Should source documents be stored locally in the app database, referenced by URL/vendor ID, or cached as normalized text?
- Let's capture source docs in our own DB, and obviously we'll include URL/vendor ID/etc along side it


# Core Architecture
What should be the primary domain boundary: Stock, NarrativeEpisode, SourceDocument, or ResearchMemo?
- NarrativeEpisode and Stock are by far the most important
- Users will search by stock, and ideally when they land on a stocks page they immediately see the most relevant or active episode
- We do need to acknowledge though that multiple narratives could be competing at once
- We should also be able to find similar past episodes so users can research what has happened in the past when similar narratives occurred
- The UI maybe ends up being a timeline view, where the stock page shows a timeline with the episode start/end on a graph, and you can switch between which episode your reading about

Should narrative generation be modeled as an asynchronous pipeline/job system, or can it happen request-time for early MVP?
- Async pipeline/job system.  Loco has tooling around handling that; we should stick inline with what it makes available

Should LLM outputs be persisted as reviewed domain objects, or regenerated from sources whenever requested?
- Persisted as reviewed domain objects; Our job system should generate the research artifacts and save them to the DB, vs. regenerating things on user request
- We are likely to have a fairly deep and expensive research pipeline, so we really want to save the results instead of recalculating all the time
- We also want to show users a consistent report experience, vs. every user being told a different thing

Do you want an internal admin/review workflow in the initial architecture, or just enough structure to add it later?
- TBD - I'm leaning towards yes, but I'm concerned adding a lot of admin here will make this really hard to operate
- We need a higher degree of "observability" as we're tweaking prompts and workflows, but after the prompts and workflows are well validated it should run on auto-pilot
- Eventually, we'll want a "stocks" page that lists ongoing narratives for at least the S&P 500, and we don't want to have to manually review every single one of them
- At the same time, we'll eventually need some ability to automatically detect where things might be going off the rails or require human intervention to ensure things make sense, are fact checked
- This might be a 'risk scores' type system
- We may want to automatically generate lists of all 'factual claims' made in each narrative, and ensure it has a clean high quality citation.
- We may want to automatically tag narratives as either "light" | "medium" | "heavy" on speculation, and review the "medium" | "heavy" speculation narratives more deeply
- We may want to automatically tag the "implied performance promises" of each narrative so that narratives suggesting bolder future performance claims are easy to identify and manually review
- So... Let's setup a basic admin dashboard, but focus it on detecting risky narratives rather then an approval flow for now

# Retrieval And LLM
Should similarity search be based on stored NarrativeEpisode embeddings only, or should it also search raw source chunks?
- Lets use NarrativeEpisode embeddings + other properties for now.  No raw source chunks

Where should explainable similarity live: as generated text, structured attributes, or both?
- As generated text
- We probably want to build entire report sections though that go deeper on the similar narratives mentioned
- After we identify the similar narratives, we should have an LLM look at each of those narratives, the stock performance over the course of the narrative, the narrative timeline that played out, and discuss how things are similar and different in both cases with speculation about what that might mean.  This data should be saved to the DB as part of the report.

How strict should citations be: every claim linked to source spans, or source-backed sections with looser attribution?
- A lot of claims will be pretty loose.  We're heavily leaning on secondary/tertiary sources like online blogs + discussions.
- For claims coming from authoritative sources, I think source-backed sections are good, but we shouldn't see this as an "SEC filing" level source.  Generally, narratives might be closer to "TSMC is opening a new foundry in the USA" (Easily cited), success of that foundry reduces Taiwan's ability to rely on the USA / Europe for defense against China" (Easily Cited), "it's unclear if they will be cost competitive" (Easily cited), "early reports suggest they are/aren't" (Easily cited), key future pivots are the cost-per-chip, learning curve effects, and continued US government support (harder to cite those future pivots).

# Trust, Review, And Safety
What level of human review is required before a narrative page is visible to users?
- We need dashboards to quickly identify possible low-quality content, but we want to prioritize the ability to automatically show narrative pages without human intervention

Should the system explicitly separate source-time evidence, outcome data, and modern interpretation in storage from the beginning?
- Let's review multiple designs around this.  Ideally we can isolate this choice to a single module or small group of modules without it spilling into the rest of the architecture

Are there legal/compliance constraints already known beyond “no recommendations / no personalized advice”?
- Unknown.  Nothing we're currently worried about
- We need to get a better sense of what's possible to see if there's anything worth launching at all, before we dive too deeply into legal/compliance things that can definitely be solved if this is commercially viable and solid from a product perspective


# Delivery Shape
Should the system map include crate boundaries now, or first identify seams and defer crate splitting?
- Let's look at getting crate boundaries + seams in place now

What documents should come next after the system map: crate charters, contract specs, ADRs, or task sheets?
- Crate Charters + Contract Specs are next

What would make the system map successful for you: implementation clarity, parallel-agent coordination, investor/product clarity, or all three?
- Sequencing Clarity, so it's clear what should be built and in what order
- Clean module boundaries to make agent coordination + PR review more obvious
- Module boundaries that allow for effective and isolated unit testing
- A development sequence that lets us get to an internal "Aha!" moment quickly, even if it's a bit contrived / includes manual steps / isn't fully realistic yet
- investor clarity is not a priority
- Architectural soundness is more important then parallel-agent coordination, although they often will amount to the same thing

