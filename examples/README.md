# Examples — illustrations, not standards

FerroStep deliberately ships **no canonical workflows**. The engine provides
configuration primitives — states, roles, transitions, counters, an opaque
`purpose` — and these files show shapes they can take. Copy them, edit them,
or ignore them; nothing in the engine knows they exist.

- **`review-loop.json`** — a worker/reviewer rework loop with a crash-priced
  pass ceiling that escalates to a human. Also the test suite's acceptance
  fixture.
- **`product-review.json`** — the same primitives, different subject: here the
  ledger record is not a work item but *the product itself* at a point in time
  (the application puts something like a `reviewed_ref` on the record). The
  reviewer delivers a report measuring alignment against the document named in
  `purpose`; a review is a report — what work follows it is the owner's
  decision, made outside this loop.

The pattern in one line: the ledger holds what's *true*, the purpose document
holds what's *intended*, and reviews measure one against the other.
