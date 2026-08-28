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
  decision, made outside this loop. Also the one that shows the two
  permissions `review-loop.json` leaves out, which is what makes the pair
  useful: absent from a definition, each grants the operation to **nobody**
  rather than to anybody.
  - **`grades`** — an ordered ladder, with **each direction granted
    separately**. ⚠ Deliberately the shape an adopter's intuition gets
    backwards: the gate here is *confidence must reach `high` before this is
    delivered*, so **raising is the direction that clears the gate** and it is
    the owner's, while the reviewer may only lower. The familiar shape is the
    opposite — a severity floor, where raising cannot clear anything — and an
    engine that assumed either would be silently wrong for the other in the
    direction that grants permission. So the referee guards who moves the
    value and which way, and has no opinion about which end clears your gate.
  - **`rescopes`** — a review belongs to a release line, and only the owner
    may move it to another, with a reason: a record that changes which
    queries find it and says nothing about why is indistinguishable from one
    that quietly vanished. No definition can permit it on a finished record.
  - **`creation`** — who may file, and what filing costs. `reviews_queued`
    bounds how many reviews a release line may have, which is a ceiling on a
    *population* rather than on any one record: every record stays
    individually bounded by `review_attempts` while the population grows, and
    only a filing budget stops it growing forever. ⚠ That count belongs to
    whatever can count it, so it is never stored on the record it filed — the
    spend lives in that record's history, where a caller goes to derive the
    next value.

The pattern in one line: the ledger holds what's *true*, the purpose document
holds what's *intended*, and reviews measure one against the other.
