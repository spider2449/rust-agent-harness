# ADR 0051: Loops Open a Model Request Again When the Provider Cannot Take It

Status: Accepted

Date: 2026-10-17

## Context

Providers are built with `reqwest::retry::never()`, and nothing above them
retried. A 429 or a 503 ended the turn with an error, however short the
outage, and the user had to send the message again. opencode retries with an
exponential schedule, honors `Retry-After` up to a bound and gives up after
ten tries.

RAH's rule is *never automatically replay uncertain external effects*
(AGENTS.md). A model request has no external effect: nothing is executed
until the stream completes and the loop runs the tool calls the model asked
for, and a request the provider refused before it began did nothing at all.
Retrying the **opening** is therefore safe. Retrying after output started is
not: text would be shown or recorded twice.

## Decision

1. **`ModelError::Unavailable { message, retry_after }`** is added to
   `rah-api`: the provider cannot take the request now and did nothing with
   it (HTTP 429, 500, 502, 503, 504, 529). The three HTTP providers map
   those statuses, on the request that opens the stream, to it, with the
   number of seconds of a `Retry-After` header when there is one. Other
   statuses stay `Rejected`; 401 and 403 stay authentication failures.
2. **`rah_api::retry::stream`** opens a request through a `ModelProvider` and
   opens it again after `Unavailable` or `Transport` from the opening only.
   `Rejected`, `Protocol`, `Cancelled` and every error of a stream that has
   opened are returned at once. The wait is 2, 4, 8, then 10 seconds, ten
   retries (about 84 seconds), jittered by up to 20%; `Retry-After` can only
   lengthen a wait and is honored up to 15 minutes. Cancelling ends a wait
   with `Cancelled`. When the retries are used up the last error is returned.
3. **Both loops use it** (`loop-react`, `loop-plan`), configured by
   `retries` (default 10, at most 100; `0` turns retrying off) and
   `retry_delay_ms` (default 2000; the wait doubles up to five times this).
   The helper lives in `rah-api` because plugins cannot depend on one
   another and both loops need it. Side requests that are best effort
   (`session-titles`, `context-compact`) do not retry.
4. **`AgentEvent::ModelRetrying`** (`session_id`, `model_request_id`,
   `attempt`, `retries`, `delay_ms`, `reason`) reports each wait. It carries no
   credentials: the reason is the error's text, which providers already keep
   free of headers and keys. The Web UI and the terminal show it,
   `telemetry-log` counts retries on the `model_request` record, and the
   session log does not record it, since the attempt left nothing.

## Consequences

- A rate limit or a short outage costs a pause, not a turn. A permanent
  outage costs about 84 seconds before the turn fails, with progress shown.
- A provider that wrongly answers 500 for a request it will never accept is
  tried eleven times. `retries = 0` is the opt-out.
- A request that reached the provider and was lost on the way back
  (`Transport` while opening) is sent again. The provider may bill the first
  one; nothing else can have happened.
- Frontends that match `AgentEvent` exhaustively must handle the new
  variant (the in-tree ones do).
