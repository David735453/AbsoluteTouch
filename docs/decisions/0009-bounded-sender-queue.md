# 0009: Bounded sender queue and honest latency counters

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

The first real Wi-Fi session worked but "sometimes lagged". Measured evidence:

- The desktop summary reported `last RTT: 921237 us`. That number was an
  artifact: the iPad echoes the host time from the previous `HEARTBEAT_ACK` and
  only sends its next heartbeat up to a second later, so the host measured the
  heartbeat period plus the round trip. Nothing was actually taking 921 ms.
- A recorded trace showed samples arriving in clumps of about seven every
  46 ms, with the age of the oldest sample in a clump up to 150 ms (99th
  percentile 117 ms). The samples inside a clump carried distinct sender
  timestamps, so they had been buffered somewhere between the sender and the
  receiver, not batched by the canvas.
- The desktop injector is not the bottleneck: replaying 2932 samples through
  `uinput` at full speed takes 0.43 s, about 145 us per sample.
- The laptop's Wi-Fi interface had `Power save: on` while associated with a
  phone hotspot advertising a 300 ms beacon interval, which is exactly the
  shape of the observed clumping.

Two code paths turned a transient network stall into lasting lag:

1. `SessionController` submitted samples into an unbounded `AsyncStream`. Each
   sample was then sent with an `await` on `NWConnection`'s send completion, so
   a burst of coalesced touches took several datagram round trips to drain.
   While it drained, new samples kept arriving. Because production and drain
   rates are similar, a backlog created during a stall never shrank: the cursor
   stayed behind the Pencil until the stroke ended.
2. The desktop had no way to report the delay a user actually feels, so the
   only visible number was the misleading RTT.

## Decision

- Replace the unbounded sample stream with a `SampleMailbox`: a lock-protected
  buffer capped at 32 samples, filled from the main actor and drained by one
  controller task. A full buffer drops the oldest sample, which is safe because
  every sample carries the complete pointer state.
- Coalesce each drained batch to the samples that change the pointer flags plus
  the newest position. A press, a release, a hover transition, a barrel change,
  and the eraser are always sent; redundant intermediate coordinates are not.
  This is sound for an absolute device: the compositor and the game sample the
  cursor position once per frame, and no consumer integrates deltas.
- Count the replaced samples as `samplesCoalesced` and show them in the app HUD.
- Fix the desktop RTT: subtract the sender's wait, measured on the sender's own
  clock with the device timestamp it echoes in `x`/`y`, from the host-side
  turnaround.
- Add `input latency` to the session summary: the age of each injected sample
  in host time, using the heartbeat clock offset. Report the newest and the
  worst value. Samples with an age over one second are ignored, because they
  mean a clock jump rather than a slow link.

## Consequences

- A network stall costs one frame instead of an unbounded backlog. The cursor
  catches up as soon as the link recovers, and the app's `coalesced` counter
  shows how much was dropped.
- Fewer datagrams per second: a burst of seven coalesced touches becomes one or
  two packets, which also frees airtime on a congested link.
- Intermediate positions no longer reach the desktop, so a recorded trace has
  fewer samples than the iPad produced. The state sequence is unchanged.
- `input latency` excludes the display pipeline and the game's own frame time,
  so it is a lower bound on what the user sees. It is the number to compare
  between runs, transports, and power-save settings.
- Wi-Fi power saving, access-point beacon intervals, and hotspot buffering stay
  outside the program. [Setup](../setup.md#latency) documents how to check them,
  and USB (M4) removes the radio from the path entirely.
