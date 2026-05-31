# Rust Playbook for Parallel LLM-Agent Development

## Purpose

This playbook is for building Rust products faster with LLM AGENTS by doing the minimum architecture work needed to make parallel implementation safe, mergeable, and testable.

The core idea is simple:

> **Pull forward boundary design, not full-system design.**

When you use LLM agents heavily, architecture is not just about code quality. It is also about **work partitioning**. Clear seams let multiple agents move in parallel without colliding constantly.

---

## 1. Core rule: architect the seams, not the internals

For early MVP work, you usually do not want heavy big-design-up-front.

What you *do* want early is:

* crate boundaries where coordination matters
* public contracts at those boundaries
* examples and fixtures
* contract tests
* dependency direction rules

What you usually want to delay:

* deep internal design
* speculative abstractions
* too many crates
* future-proof plugin systems
* detailed module trees inside crates

The practical standard is:

> **Do enough architecture so that 2–5 agents can work in parallel without repeatedly touching the same files or inventing incompatible assumptions.**

---

## 2. Crate vs seam

These are related, but not identical.

### Crate

A **crate** is a Rust packaging and compilation boundary. It is useful when you want a stronger enforcement boundary around dependencies, ownership, and public API exposure.

A crate is a good choice when you need:

* a real change boundary
* clear ownership
* protection from accidental coupling
* reuse across multiple parts of the system
* different rates of change between areas
* a stable public surface

### Seam

A **seam** is any meaningful interface where work can be split and validated independently.

A seam might be:

* a crate boundary
* a trait
* a protocol/schema boundary
* an adapter normalization boundary
* a storage interface
* an event stream contract

Not every seam needs its own crate. Early on, many seams can live inside the same crate.

### Practical rule

Use a **crate** when you need stronger coordination and dependency control.
Use a **seam** whenever multiple agents or subsystems need a stable contract.

A good early architecture often has **fewer crates than seams**.

---

## 3. External vs internal architecture

This is one of the most important distinctions.

### External architecture

External architecture is what other parts of the system are allowed to rely on.

This includes:

* public types
* public traits
* public functions
* DTOs and serialized schemas
* error surfaces
* config shape
* invariants
* ordering/timing/determinism expectations where relevant

This is where you should be precise.

### Internal architecture

Internal architecture is what you want freedom to change without forcing the rest of the system to change.

This includes:

* internal modules
* helper types
* algorithms
* storage details
* caching choices
* refactors
* implementation patterns

This is where you should stay looser, especially early.

### Guideline

> **Specify the border tightly. Leave the interior flexible.**

That gives agents enough structure to coordinate without freezing the design too early.

---

## 4. What to define early

For each important crate or seam, get specific about:

* purpose
* responsibilities
* non-responsibilities
* public entry points
* dependency direction
* invariants
* error behavior
* concrete examples
* required tests

This is the high-leverage architecture work for agent-heavy development.

You do **not** need to fully specify:

* exact internal module layout
* helper abstractions
* detailed implementation structure
* optimization plans
* speculative extensibility

---

## 5. Required docs

Keep the artifact set small but strong.

### 1. System map

A one-page view of:

* major crates
* major seams
* dependency arrows
* runtime components
* key data flows

This aligns everyone on the shape of the system.

### 2. Crate charters

One short document per important crate.

Include:

* purpose
* owns
* does not own
* allowed dependencies
* forbidden dependencies
* public entry points

This defines ownership and boundaries.

### 3. Contract specs

One short document per important seam.

Include:

* public types or traits
* behavioral expectations
* invariants
* ordering/timing assumptions
* error semantics
* valid and invalid examples

This is the main coordination artifact for parallel work.

### 4. ADRs

Use ADRs for cross-cutting structural decisions that affect multiple crates or seams.

Examples:

* why a protocol crate exists
* why vendor data is normalized at adapter boundaries
* why a replay engine is event-driven
* why timestamps are modeled a specific way

### 5. Task sheets

A short implementation brief for each agent task.

Include:

* objective
* what to read first
* allowed changes
* forbidden changes
* required tests
* definition of done

This keeps scope narrow and mergeable.

---

## 6. How to test crates and seams

This is where many agent workflows break down. The types compile, but the system still does not integrate.

### Testing seams

Seams need **contract tests**.

Typical seam tests:

* round-trip serialization tests
* trait conformance tests
* valid/invalid fixture tests
* determinism tests
* ordering tests
* normalization tests
* compatibility/golden tests

The point is to test behavior at the boundary, not just structure.

### Testing crates

Crates need tests for their internal logic and local integration.

Typical crate tests:

* unit tests for domain logic
* focused integration tests inside the crate
* property tests where helpful
* smoke tests for main public entry points

### Rule of thumb

> **Seams are tested for contract correctness. Crates are tested for implementation correctness.**

If a boundary matters for parallel work, it should have executable tests tied directly to the contract.

---

## 7. How much architecture to do in early MVP phases

Using LLM agents heavily does mean you should do **more boundary architecture earlier** than you would in a pure solo-hacker workflow.

But it does **not** mean you should do full architecture earlier everywhere.

What changes is this:

* define boundaries earlier
* define contracts earlier
* define fixtures earlier
* define dependency rules earlier
* define contract tests earlier

What should still stay light:

* internal implementation plans
* deep abstraction layers
* fine-grained crate splitting
* speculative generalization

The best framing is:

> **LLM-heavy development pulls forward seam architecture, not full-system architecture.**

---

## 8. Templates

### Crate charter template

```md
# Crate Charter: <crate_name>

## Purpose
<one paragraph>

## Owns
- ...
- ...

## Does Not Own
- ...
- ...

## Public Entry Points
- ...
- ...

## Allowed Dependencies
- ...
- ...

## Forbidden Dependencies
- ...
- ...

## Key Invariants
- ...
- ...
```

### Contract spec template

```md
# Contract Spec: <seam_name>

## Overview
<what this seam is for>

## Public Types / Traits
- ...
- ...

## Operations
- ...
- ...

## Behavioral Rules
- ...
- ...

## Error Semantics
- ...
- ...

## Valid Examples
- ...
- ...

## Invalid Examples
- ...
- ...

## Required Tests
- ...
- ...
```

### Task sheet template

```md
# Task: <task_name>

## Objective
...

## Read First
- ...
- ...

## Allowed Changes
- ...
- ...

## Do Not Change
- ...
- ...

## Tests
- must pass ...
- must add ...

## Definition of Done
- ...
- ...
```

---

## 9. Final operating principles

1. **Use architecture to reduce coordination cost, not to maximize elegance.**

2. **Pull forward seam design, not deep internal design.**

3. **Define crates around change boundaries and work boundaries.**

4. **Specify borders tightly; keep interiors adaptable.**

5. **Use fixtures and contract tests as first-class architecture tools.**

6. **Prefer concrete examples over abstract guidance.**

7. **Keep parallel work small-batch and frequently reintegrated.**

8. **Split crates only when it solves a real coordination problem.**

9. **Treat agents like teammates who need explicit boundaries and executable expectations.**

10. **Optimize for mergeability and correctness, not just generation speed.**

---

## Summary

The winning pattern for Rust + LLM agents is:

* keep the workspace fairly simple
* identify the important seams early
* define clear external contracts
* keep internal design flexible
* back important seams with fixtures and contract tests
* give agents narrow tasks
* merge in small batches

That is usually enough structure to unlock real parallelism without over-architecting the MVP.
