# ADR 0051: Loops Open a Model Request Again When the Provider Cannot Take It

Status: Accepted

Date: 2026-10-17

Amended to recover interrupted llama.cpp streams using the existing loop
retry budget.

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
Retrying the opening is safe. Retrying an interrupted stream also requires
discarding its provisional output and any buffered tool calls.

## Decision

1. **`ModelError::Unavailable { message, retry_after }`** is added to
   `rah-api`: the provider cannot take the request now and did nothing with
   it (HTTP 429, 500, 502, 503, 504, 529). The three HTTP providers map
   those statuses, on the request that opens the stream, to it, with the
   number of seconds of a `Retry-After` header when there is one. Other
   statuses stay `Rejected`; 401 and 403 stay authentication failures.
2. **`rah_api::retry::stream`** opens a request through a `ModelProvider` and
   opens it again after `Unavailable` or `Transport` from the opening only.
   `Rejected`, `Protocol` and `Cancelled` are returned at once.
   Both loops also use `rah_api::retry::complete` to consume a stream until
   completion. `model-llamacpp` reports an interrupted response body as
   `ModelError::StreamInterrupted`. For that error the loops discard the
   attempt and resend the same model request, including any tool results
   already in its context. Opening failures and interruptions share one
   retry budget. The wait is 2, 4, 8, then 10 seconds, ten
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
   `attempt`, `retries`, `delay_ms`, `reason`) reports each wait. The optional
   `discard_partial` field tells consumers to discard provisional output from
   the failed attempt. The Web UI removes it and the terminal marks it as
   discarded. The reason is the provider's error text, which providers keep
   free of headers and keys. Both frontends show the wait, and
   `telemetry-log` counts retries on the `model_request` record. The session
   log records only completed output from that model request.

## Consequences

- The default retry schedule adds about 84 seconds of waiting across
  attempts, plus the time spent on the requests themselves.
- A provider that wrongly answers 500 for a request it will never accept is
  tried eleven times. `retries = 0` is the opt-out.
- The provider may bill more than one attempt if it started generation
  before the connection failed.
- Frontends consuming the serialized `ModelRetrying` event can recognize
  `discard_partial` to remove abandoned output for the same request ID.
