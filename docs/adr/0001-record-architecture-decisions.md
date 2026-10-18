# 0001. Record architecture decisions

- Status: accepted
- Date: 2026-10-18

## Context

Requirements section 0 requires every non-trivial choice to be logged as an ADR, and section 2 says
the agent decides ambiguities and records them.

## Options

1. ADRs in `docs/adr/NNNN-title.md`, one per decision (as required).
2. A single running decision log. Harder to link from the spec and to supersede.

## Decision

Option 1, using `0000-template.md`. Numbers are sequential and never reused; superseded ADRs stay and
are marked as such.

## Consequences

Each milestone's progress entry links the ADRs it created.
