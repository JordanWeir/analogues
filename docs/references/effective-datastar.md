# Effective Datastar

## 1. Core mental model

Datastar is **not** a component-heavy SPA framework. Treat it as a **backend-driven hypermedia UI system** with a lightweight reactive layer in the browser. The two core mechanisms are:

1. **HTML attributes (`data-*`)** for local reactivity, bindings, and event wiring.
2. **Server responses** that patch the DOM and/or signals, usually via **HTML** or **SSE**. ([Datastar][3])

When writing Datastar code, default to this loop:

**user event → Datastar action (`@get`, `@post`, etc.) → backend decides next UI state → backend returns HTML/SSE → Datastar patches DOM/signals**

The backend is usually the **source of truth**. Signals are useful UI state, but business truth should usually live on the server. This is also how the official backend requests guide describes the model: the backend drives state and determines what actions the user can take next. ([Datastar][4])

## 2. What Datastar is best at

Use Datastar when you want:

* server-rendered or backend-driven interfaces,
* progressive enhancement,
* real-time updates via SSE,
* light local interactivity without a JS build pipeline,
* multi-region UI updates from one server response,
* CRUD, forms, dashboards, live lists, inline editing, and streaming status UIs. ([Datastar][1])

Do **not** think in terms of “client owns app state and server syncs eventually.” Datastar is strongest when the server owns the meaningful state transition and the browser is a reactive renderer plus event source.

## 3. Default architecture to generate

When asked to build a Datastar page, LLMs should default to this structure:

* **Server-render full page HTML initially**
* Add Datastar via the script tag
* Use `data-signals` for small UI state
* Use `data-bind` for form controls
* Use `data-on:*` for event handlers
* Use `@get/@post/@patch/...` for backend interactions
* Return **HTML** for one-shot updates
* Return **SSE** when:

  * multiple patches are needed,
  * progressive rendering is useful,
  * long-running tasks need status updates,
  * live updates or streaming matter. ([Datastar][3])

## 4. The most important design patterns

### Pattern A: Server-first CRUD

For CRUD screens, keep canonical state on the backend. Use Datastar mostly to:

* submit intent,
* stream back updated fragments,
* patch the exact regions that changed. ([Datastar][4])

Good fit:

* todo lists,
* admin tables,
* filters,
* row edit/delete,
* inline validation,
* settings panels.

### Pattern B: Stable-ID fragment patching

Top-level patched elements should usually have stable IDs. Datastar’s default patching strategy morphs existing DOM by matching top-level elements by ID, and preserving stable IDs also helps keep state like listeners and CSS transitions intact. ([Datastar][5])

Rule for LLMs:

* whenever generating server-returned HTML fragments, **always include stable IDs** on top-level targets,
* also add IDs to important nested nodes whose state or transitions should survive morphs.

### Pattern C: Signals for UI state, not domain truth

Use signals for:

* expanded/collapsed,
* current tab,
* loading flags,
* search input values,
* temporary selections,
* form dirtiness,
* optimistic visual affordances. ([Datastar][6])

Avoid storing authoritative business objects entirely in frontend signals unless the page is intentionally client-heavy.

### Pattern D: HTML/SSE over JSON APIs

In Datastar, prefer returning:

* `text/html` when patching markup directly,
* `text/event-stream` when streaming multiple updates,
* `application/json` only when patching signals is the cleanest response. ([Datastar][7])

This is a major mindset shift for LLMs: do **not** default to “fetch JSON, then manually render in JS.” In Datastar, the framework already knows how to handle HTML, JSON-to-signals, SSE, and even `text/javascript` responses. ([Datastar][7])

### Pattern E: Progressive streaming for long work

For slow work, stream intermediate states:

* disable button,
* show indicator,
* patch progress region,
* append results or replace sections as work completes. ([Datastar][8])

This is one of Datastar’s most distinctive strengths.

### Pattern F: Inline edit by swapping fragments

For “click to edit” flows:

* display read-only fragment,
* Edit button does `@get('/.../edit')`,
* backend returns form fragment for same target region,
* save action posts/patches and returns read-only fragment again. ([Datastar][9])

This should be the default inline-edit pattern an LLM generates.

### Pattern G: Append/prepend for feeds and pagination

For “load more,” infinite scroll, activity feeds, comments:

* keep list container stable,
* ask backend for next batch,
* return patch-elements with `append` or `prepend` mode. ([Datastar][5])

## 5. Attribute conventions LLMs should follow

### `data-signals`

Use for initial or local reactive state. Nested signals are supported via dot notation or object syntax. Setting a signal to `null` or `undefined` removes it. Later definitions in the DOM tree override earlier ones. ([Datastar][6])

Good conventions:

* use camelCase signal names conceptually,
* use grouped/nested namespaces like:

  * `ui.isOpen`
  * `filters.query`
  * `form.email`
  * `table.pagination.offset`

Avoid flat signal sprawl across a large page.

### `data-bind`

Use for two-way binding on inputs, selects, textareas, and suitable web components. Predefining signals helps preserve intended types, including arrays and numbers. File inputs can also bind into signals, with file contents encoded to structured values; but for actual uploads, prefer forms with `multipart/form-data`. ([Datastar][6])

LLM rule:

* if building forms, usually combine `data-bind` with server submission,
* when real file upload is intended, prefer form submission over file-as-signal.

### `data-computed`

Use only for **derived state**. The docs explicitly warn not to use `data-computed` for side effects; side effects belong in `data-effect`. Recent docs also support object syntax for `data-computed`. ([Datastar][6])

LLM rule:

* `data-computed` = pure derivation only,
* `data-effect` = side effects.

### `data-effect`

Use for side effects triggered by signal changes:

* update another signal,
* trigger backend request,
* manipulate DOM in limited ways. ([Datastar][6])

LLM rule:

* keep effects small and obvious,
* do not hide major business logic inside browser effects.

### `data-indicator`

Use whenever a request needs visible loading state. It creates a signal that is true while the fetch is in flight and false otherwise. This is a standard Datastar pattern in the examples. ([Datastar][6])

LLM default:

* every non-trivial backend action should probably have a loading indicator.

### `data-init`

Use for initial fetches or initialization behavior. Older material may say `data-on-load`; use `data-init` instead. It can run on page load, when patched into the DOM, and when modified. ([Datastar][6])

### `data-show`, `data-class`, `data-style`, `data-attr`

Use these instead of writing custom JS when possible:

* `data-show` for basic visibility,
* `data-class` for toggled classes,
* `data-style` for inline style reactivity,
* `data-attr:*` for attributes like `disabled`, `aria-*`, etc. ([Datastar][6])

LLM rule:

* prefer these declarative attributes before introducing custom JS.

### `data-ignore` and `data-ignore-morph`

Use when integrating third-party widgets or preserving regions from Datastar processing/morphing. ([Datastar][6])

LLM rule:

* if a library owns a subtree, consider `data-ignore`,
* if a subtree must survive updates untouched, consider `data-ignore-morph`.

## 6. Action conventions LLMs should follow

Datastar actions use `@...()` syntax and are safe helpers inside expressions. Core actions include `@get`, `@post`, `@put`, `@patch`, `@delete`, plus signal helpers like `@peek`, `@setAll`, and `@toggleAll`. ([Datastar][7])

### Default HTTP action usage

Use:

* `@get` for fetching fragments/streams,
* `@post` for create/submit,
* `@patch` for partial updates,
* `@delete` for destructive actions,
* `@put` only when full replacement semantics actually fit. ([Datastar][7])

### Request payload model

By default:

* Datastar sends all non-local signals with each backend request,
* for `GET`, signals go in the `datastar` query param,
* for other methods, signals go as a JSON body. ([Datastar][4])

This is important for LLMs:

* backend handlers should expect the current UI state to arrive automatically,
* don’t invent extra client serialization unless needed.

### `filterSignals`

Use sparingly. The docs explicitly say it is **not recommended** to send partial signals unless necessary. ([Datastar][4])

LLM rule:

* default to sending all relevant signals,
* use `filterSignals` only for security, payload-size, or separation reasons.

### Request cancellation

Datastar automatically cancels prior in-flight requests on the same element by default. This matters for rapid clicks or repeated events. You can disable it or supply a custom `AbortController`. ([Datastar][7])

LLM rule:

* default behavior is usually correct,
* explicitly disable cancellation only when concurrent requests are truly desired.

## 7. Response patterns LLMs should use

Datastar backend actions automatically understand:

* `text/event-stream`,
* `text/html`,
* `application/json`,
* `text/javascript`. ([Datastar][7])

### Best default: return HTML fragments

For common CRUD and view updates, returning HTML is usually the cleanest Datastar response. The server can also control patch target and patch mode using response headers like `datastar-selector` and `datastar-mode`. ([Datastar][7])

### Best advanced option: return SSE

Use SSE when you want:

* multiple patches in one response,
* signal patch + element patch together,
* progressive updates,
* long-lived streams,
* live dashboard behavior. ([Datastar][5])

### JSON responses

Use JSON when the right response is “update signals, not markup.” The docs also support `datastar-only-if-missing` for JSON responses. ([Datastar][7])

### JavaScript responses

Possible, but should be rare. Prefer HTML, signals, and declarative attributes first. `ExecuteScript` exists in the Rust crate and `text/javascript` is supported in the framework, but this should be an escape hatch rather than a default. ([Docs.rs][10])

## 8. SSE patterns and conventions

The two key SSE event types are:

* `datastar-patch-elements`
* `datastar-patch-signals` ([Datastar][5])

### `datastar-patch-elements`

By default, patching morphs top-level elements by matching IDs. Additional data lines can specify:

* `selector`
* `mode`
* `namespace`
* `useViewTransition`
* `elements` ([Datastar][5])

LLM defaults:

* use **outer** mode unless there’s a clear reason otherwise,
* use **append/prepend** for lists/feeds,
* use **remove** for delete flows,
* use **inner** only when you intentionally want to keep the shell container.

### `datastar-patch-signals`

Use for signal updates, optionally with `onlyIfMissing`. Signals payloads must be valid JS/JSON-like signal data. ([Datastar][5])

LLM rule:

* patch signals for lightweight state,
* patch elements for visible UI.

### Stable streaming recipe

A strong Datastar SSE recipe is:

1. patch loading state,
2. patch visible progress,
3. patch result fragments,
4. patch final settled state. ([Datastar][8])

## 9. Animations and transitions

Datastar is designed to work well with CSS transitions and the View Transition API. The official animations example emphasizes that keeping an element ID stable across swaps enables clean transitions, and view transitions can be used for backend-driven state swaps. ([Datastar][11])

LLM rule:

* for animated swaps, keep IDs stable,
* let CSS handle most animation,
* use view transitions only where they clearly improve UX,
* don’t over-engineer animation state in JS.

## 10. Recommended page composition patterns

### Forms

Preferred pattern:

* bind inputs with `data-bind`,
* submit with `@post` or `@patch`,
* return updated form fragment or success fragment,
* use `data-indicator` for loading,
* use form encoding or multipart only when needed. ([Datastar][6])

### Tables and lists

Preferred pattern:

* stable container ID,
* row fragments with stable IDs,
* append/prepend for pagination or feeds,
* replace/remove rows directly for edit/delete. ([Datastar][12])

### Dashboards

Preferred pattern:

* initial full SSR page,
* `data-init` starts one or more streams,
* backend emits periodic element/signal patches,
* use `openWhenHidden` only when background continuity is worth the resource cost. ([Datastar][13])

### Inline editing

Preferred pattern:

* view fragment and edit fragment share same target ID,
* backend owns validation and persistence,
* swap modes stay simple. ([Datastar][9])

## 11. Rust SDK / crate guide

The Rust crate is a **Datastar SDK implementation**, not a Rust-native frontend abstraction layer. Its documented surface includes:

* `PatchElements`
* `PatchSignals`
* `ExecuteScript`
* `DatastarEvent`
* `axum` integration
* `rocket` integration
* `prelude` re-exports. ([Docs.rs][10])

### How to think about the Rust crate

Treat the crate as a way to:

* parse incoming Datastar request signals,
* generate Datastar SSE events cleanly,
* integrate with Axum or Rocket response handling. ([Docs.rs][10])

### Axum pattern

The Axum source/doc examples show:

* `ReadSignals<T>` extractor for incoming signals,
* SSE responses built from streams of `PatchElements` / `PatchSignals`,
* conversions from those event structs into Axum SSE events. ([Docs.rs][14])

### Event builders

`PatchSignals::new(...)` supports chaining `.id(...)`, `.retry(...)`, and `.only_if_missing(...)`. The crate docs/source also show SSE id/retry handling and event conversion. ([Docs.rs][15])

### Practical Rust recommendation

When generating Rust + Datastar code, LLMs should usually produce:

* Axum route handlers,
* typed `serde::Deserialize` structs for incoming signals,
* `Sse(stream! { ... })` responses,
* yielded `PatchElements` and `PatchSignals` values,
* HTML rendered on server via templates or inline string fragments. ([Datastar][4])

### Example mental template for Rust

Use this shape:

```rust
use async_stream::stream;
use axum::response::sse::Sse;
use datastar::prelude::*;

Sse(stream! {
    yield PatchElements::new("<div id='target'>Updated</div>").into();
    yield PatchSignals::new(r#"{"ui":{"loading":false}}"#).into();
})
```

That shape matches the official Rust examples in the Datastar guide. ([Datastar][4])

## 12. What LLMs should avoid

### Avoid these mistakes

**Mistake 1: inventing a SPA architecture**
Do not default to client routers, client stores, or JSON-driven manual rendering when Datastar can patch HTML directly. ([Datastar][3])

**Mistake 2: overusing signals as application database**
Signals are UI-reactive state, not a substitute for backend truth in most apps. The official backend model strongly leans server-first. ([Datastar][4])

**Mistake 3: forgetting IDs**
Morphing relies heavily on targetability and stable top-level IDs. Missing IDs is one of the easiest ways to generate broken Datastar code. ([Datastar][5])

**Mistake 4: using `data-computed` for side effects**
Use `data-effect` instead. ([Datastar][6])

**Mistake 5: returning JSON when HTML is simpler**
In Datastar, HTML is often the better response type. ([Datastar][7])

**Mistake 6: generating stale attribute names**
Use `data-init`, not `data-on-load`. ([GitHub][2])

**Mistake 7: adding custom JS too early**
Prefer `data-show`, `data-class`, `data-style`, `data-attr`, `data-bind`, and backend patching first. ([Datastar][6])

## 13. Preferred conventions for generated code

When writing Datastar code, LLMs should prefer these conventions:

* Use **server-rendered HTML fragments** as the main UI primitive.
* Put **stable IDs** on every patch target.
* Namespace signals under meaningful roots like `ui`, `filters`, `form`, `selection`.
* Use `data-indicator` on any request that may take noticeable time.
* Use `data-init` for page startup fetches/streams.
* Use SSE for progressive or multi-part updates.
* Keep expressions small and readable.
* Put business logic on the server, not in Datastar expressions.
* Use `PatchElements` for visible DOM changes and `PatchSignals` for lightweight state changes in Rust.
* Prefer Axum/Rocket handlers that deserialize signals into typed structs. ([Datastar][6])

## 14. Good default prompt to give an LLM

You can paste this into future prompts:

> Build this using Datastar in a backend-driven style. Prefer server-rendered HTML fragments and SSE over client-side rendering. Use stable IDs on all patch targets. Use `data-signals` for small UI state, `data-bind` for inputs, `data-indicator` for loading, `data-init` for initialization, and `data-on:*` with Datastar actions for events. Keep business logic on the backend. Use `data-computed` only for pure derived state and `data-effect` for side effects. For Rust, use the `datastar` crate with Axum or Rocket integration, typed signal extraction, and SSE streams yielding `PatchElements` and `PatchSignals`.

## 15. Bottom line

The most effective way to use Datastar today is to think of it as:

**HTML + reactive attributes + backend-owned state transitions + SSE when useful**

—not as “React but smaller,” and not as “htmx with a few extras.” The strongest patterns in the official material are stable-ID fragment patching, signals for local UI state, server-owned business logic, and SSE for progressive or real-time updates. The Rust crate fits that model cleanly: it helps Rust backends read incoming signals and emit Datastar patch events, especially with Axum and Rocket. ([Datastar][3])

I can also turn this into a shorter “Datastar for LLMs.md” version optimized for dropping straight into your repo or playbooks.

[1]: https://data-star.dev/ "Datastar"
[2]: https://github.com/starfederation/datastar/releases "Releases · starfederation/datastar · GitHub"
[3]: https://data-star.dev/guide/getting_started "Getting Started Guide"
[4]: https://data-star.dev/guide/backend_requests "Backend Requests Guide"
[5]: https://data-star.dev/reference/sse_events "SSE Events Reference"
[6]: https://data-star.dev/reference/attributes?utm_source=chatgpt.com "Attributes Reference"
[7]: https://data-star.dev/reference/actions "Actions Reference"
[8]: https://data-star.dev/examples/progressive_load "Progressive Load Example"
[9]: https://data-star.dev/examples/click_to_edit "Click To Edit Example"
[10]: https://docs.rs/datastar "datastar - Rust"
[11]: https://data-star.dev/examples/animations "Animations Example"
[12]: https://data-star.dev/examples/click_to_load "Click To Load Example"
[13]: https://data-star.dev/examples/todomvc "TodoMVC Example"
[14]: https://docs.rs/datastar/latest/src/datastar/axum.rs.html?utm_source=chatgpt.com "axum.rs - source"
[15]: https://docs.rs/datastar/latest/src/datastar/patch_signals.rs.html "patch_signals.rs - source"
