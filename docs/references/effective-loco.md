# Effective Loco

This note summarizes the default feature set and working philosophy of [Loco](https://loco.rs/) based on the main site and core docs pages, especially:

- [Homepage](https://loco.rs/)
- [Guide](https://loco.rs/docs/getting-started/guide)
- [Models](https://loco.rs/docs/the-app/models)
- [Controllers](https://loco.rs/docs/the-app/controller)
- [Workers](https://loco.rs/docs/processing/workers)
- [Mailers](https://loco.rs/docs/processing/mailers)
- [Tasks](https://loco.rs/docs/processing/task)
- [Scheduler](https://loco.rs/docs/processing/scheduler)
- [Authentication](https://loco.rs/docs/extras/authentication)

The goal is not to restate every doc page. It is to capture how Loco "wants" you to build apps so you can work with the framework instead of against it.

## One-sentence mental model

Loco is a productivity-first, Rails-inspired Rust web framework that tries to give you a default app architecture, code generators, operational tooling, and common product infrastructure out of the box so you can spend most of your time writing domain logic.

## Core philosophy

The strongest themes repeated across the site and docs are:

### 1. Convention over configuration

Loco makes a lot of decisions for you up front:

- folder structure matters
- config shape matters
- app wiring matters
- generators assume the standard structure

This is intentionally anti-analysis-paralysis. The docs are explicit that you should build the app instead of spending time inventing abstractions or architecture patterns from scratch.

### 2. Fat models, slim controllers

This is probably the clearest architectural preference in Loco.

- models are meant to hold business logic, validation, relations, and workflows
- controllers should mostly understand HTTP, parse inputs, call domain logic, and shape responses

If you come from Rails, this should feel familiar. If you come from more handler-heavy Rust frameworks, this is an important mindset shift.

### 3. Command-line driven development

Loco strongly prefers generation over manual boilerplate:

- generate models
- generate migrations
- generate controllers
- generate scaffolds
- generate tasks
- generate workers
- generate mailers
- generate scheduler config

The framework is clearly designed around keeping developer momentum high through generators and predictable file placement.

### 4. Infrastructure-ready building blocks

Loco presents common app concerns as first-class, built-in concepts:

- controllers
- models
- views
- tasks
- background jobs
- mailers
- scheduler
- authentication

The message from the docs is: most startup or internal-tool infrastructure should already have a home in the framework.

### 5. Local-first productivity

The homepage leans hard on the idea that you can build a serious app locally without immediately depending on outside SaaS products:

- local DB
- local worker modes
- local SMTP catcher for mail
- built-in auth flows
- local scheduler config

This does not mean Loco is anti-cloud. It means the default dev experience is meant to be self-contained and fast.

## Default feature set

From the site and docs, Loco's default value proposition looks like this.

### App structure

A generated app gives you a conventional layout for:

- `src/controllers/`
- `src/models/`
- `src/views/`
- `src/workers/`
- `src/mailers/`
- `src/tasks/`
- `src/fixtures/`
- `tests/`
- `config/`
- `src/app.rs` as the main registration/wiring point

That structure is part of the framework contract, not just a suggestion.

### HTTP layer

Controllers are built on top of `axum`, but Loco wraps the experience in its own conventions:

- route registration through a Loco routes API
- controller functions that receive `AppContext`
- response helpers for text, JSON, rendering, etags, cookies, and more

The docs and homepage position controllers as thin request adapters, not the place for most business rules.

### Models and database

Loco uses `SeaORM` behind an Active Record-style model layer.

The intended default model experience includes:

- entity generation from schema
- migration generation
- relation handling
- validation
- business logic on model extensions
- minimal SQL for common CRUD work

The docs explicitly encourage keeping domain logic in models so the same logic can be reused from controllers, tasks, workers, and other framework entry points.

### Views and rendering

Loco supports multiple presentation styles:

- JSON responses for APIs
- server-rendered templates with Tera
- frontend-integrated/fullstack setups

The homepage frames this as flexible rather than ideological: you can render server-side, serve an API, or combine with a frontend app.

### Background jobs and workers

Background processing is a core feature, not an add-on.

Loco supports multiple worker/queue modes:

- in-process async jobs using Tokio
- SQLite-backed queue
- Postgres-backed queue
- Redis-backed queue

An important design choice is that your worker code is mostly independent of the queue backend. The docs compare this idea to Rails ActiveJob: enqueue and perform jobs through the framework abstraction, then swap backend by configuration rather than rewriting job code.

Loco also supports:

- worker generation
- worker registration in app hooks
- strongly typed worker args
- tag-based worker/job routing
- standalone worker processes or combined server+worker mode

### Tasks

Tasks are positioned as a major operational tool, not a niche feature.

The docs describe them as ideal for:

- data fixes
- report generation
- one-off business operations
- backend automation
- work you do not want to expose through a UI

This is a very Rails/Rake-like mindset. In Loco, a task is a normal and encouraged place for business operations and maintenance jobs.

### Scheduler

The scheduler is another first-class feature.

Key characteristics:

- can run tasks or shell commands
- configured in YAML
- supports human-readable schedules and cron syntax
- can run standalone or together with the main app
- supports tags and named job execution

The docs explicitly pitch it as a friendlier replacement for traditional crontab management.

### Mailers

Mail is integrated with the background worker system.

Default assumptions:

- mail is enqueued, not sent inline
- SMTP is configured in environment YAML
- mailers have a fixed, opinionated template structure
- local development can use a mail catcher like MailHog or `mailtutan`
- tests can stub deliveries and inspect them

This is a clean example of Loco's philosophy: common product infrastructure should already have a standard place and workflow.

### Authentication

Authentication is treated as a built-in framework capability, especially in the SaaS starter.

The docs highlight out-of-the-box endpoints and flows for:

- register
- login
- email verification
- forgot password
- reset password
- current user

For SaaS-style apps, Loco is clearly aiming to get you to a functioning auth-backed product quickly rather than making you assemble auth from scratch.

### Deployment helpers

The homepage and CLI point to generated deployment setup for targets like:

- Docker
- Shuttle
- Nginx

This fits the overall pattern: Loco does not only help you write handlers and models, it tries to cover the "ship the app" path too.

## What Loco is optimizing for

After reading the site and docs, Loco seems optimized for these priorities.

### Speed of shipping

The framework is deeply optimized around reducing startup friction:

- generators instead of hand-writing boilerplate
- conventional project layout
- batteries included for common app concerns
- starter templates with substantial functionality

### Product-oriented full-stack backend work

Loco is not just an HTTP router or a minimalist web toolkit. It wants to be the main operating environment for building an actual product backend, including operational concerns.

### Rust without excessive framework cleverness

The guide explicitly says you only need beginner to moderate-beginner Rust, and that Loco avoids requiring users to understand "crazy lifetime twisters" or overly magical macros.

That suggests an intentional positioning:

- more guided than low-level Rust web stacks
- less abstraction-heavy than frameworks that require a lot of custom architecture decisions

### Predictable app organization

A lot of the productivity story depends on predictability:

- every concern has a default home
- generated code lands in expected places
- hooks register framework components centrally
- config is environment-oriented and structured

This makes the app easier to navigate and easier to extend consistently.

## How to work effectively in Loco

If you want to align with the framework instead of fighting it, the docs imply these habits.

### Use generators first

When creating new app capabilities, default to generators before hand-writing files:

- `generate model`
- `generate migration`
- `generate scaffold`
- `generate controller`
- `generate task`
- `generate worker`
- `generate mailer`
- `generate scheduler`

Generated code is part of the framework's teaching mechanism. It shows you the expected shape.

### Keep controllers boring

A good Loco controller should mostly:

- accept request data
- validate/request-shape as needed
- call model or task logic
- return JSON, text, or rendered output

If controller files start accumulating business rules, you are probably drifting away from Loco's intended structure.

### Put business workflows where they can be reused

Loco encourages reuse of the same domain logic from multiple entry points:

- HTTP controllers
- tasks
- workers
- scheduler jobs

That usually means moving real business logic into models or adjacent domain modules rather than tying it to a route handler.

### Use tasks for operational or one-off work

Do not immediately build an admin endpoint for every internal operation.

In Loco, tasks are a first-class answer for:

- data repair
- imports/exports
- maintenance jobs
- business scripts
- manual workflow triggers

That is often simpler, safer, and faster than adding UI or HTTP endpoints.

### Use workers for anything slow or external

If something is:

- slow
- retryable
- IO-heavy
- fan-out oriented
- email-related

it probably belongs in a worker rather than inline inside a request.

### Let configuration choose operational mode

Loco's background processing and scheduler design strongly encourage separating business intent from runtime wiring:

- write workers against framework abstractions
- choose queue backend in config
- choose scheduler source in config
- choose environment behavior through YAML

This is one of the more powerful parts of the framework.

### Lean into the starter capabilities

If your app resembles a SaaS product, Loco's starter defaults are not accidental scaffolding. They are part of the framework's intended fast path:

- auth
- email
- DB-backed models
- workers
- scheduler
- deployment setup

The docs suggest you should exploit those defaults unless you have a strong reason not to.

## Good default architecture instincts for a Loco app

Based on the docs, a solid default approach is:

- use models as the center of domain behavior
- keep controllers thin and protocol-focused
- use workers for asynchronous workflows
- use tasks for manual or operational workflows
- use scheduler for recurring workflows
- use mailers for email workflows rather than ad hoc SMTP code
- keep environment-specific behavior in `config/*.yaml`
- prefer generated structure over custom layout inventions

This gives you a clean separation between:

- request handling
- domain logic
- asynchronous processing
- operational automation
- recurring automation

without inventing a large architecture framework yourself.

## What to avoid

The docs do not state these as hard prohibitions, but they strongly imply these anti-patterns.

### 1. Treating Loco like just another thin Axum wrapper

You can technically do that, but you would miss a lot of the value:

- generators
- tasks
- workers
- scheduler
- mailers
- auth
- framework conventions

### 2. Packing domain logic into controllers

That works for tiny examples, but it conflicts directly with the framework's stated "fat models, slim controllers" guidance.

### 3. Replacing framework conventions too early

If you immediately invent a custom project layout, custom generation flow, or custom infra abstractions, you lose the main productivity benefit Loco is trying to provide.

### 4. Building UI/admin surfaces for every internal operation

Loco gives you tasks and scheduler for a reason. Many internal workflows do not need to become HTTP features.

### 5. Binding app logic tightly to a specific queue backend

Loco's worker model is designed to abstract backend choice. If you bypass that abstraction too early, you lose one of the nicer operational properties of the framework.

## Practical reading of Loco for this repo

For a project like this one, the most important takeaway is:

Loco is not just a web server framework. It is a product-backend framework with opinions about how to organize:

- web endpoints
- data models
- async jobs
- scheduled automation
- operational scripts
- email flows
- auth

So the "effective" way to use it is to map each new capability into the right built-in concept instead of defaulting everything into controllers or miscellaneous modules.

## Short version

If you remember only a few things, remember these:

- use generators heavily
- keep controllers thin
- put business logic in models or reusable domain code
- use tasks for manual/internal operations
- use workers for async workflows
- use scheduler for recurring workflows
- let config choose infrastructure details
- lean on the batteries-included defaults before inventing custom structure
