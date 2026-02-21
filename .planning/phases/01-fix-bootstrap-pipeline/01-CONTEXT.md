# Phase 1: Fix Bootstrap Pipeline - Context

**Gathered:** 2026-02-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix the runtime0 → runtime1 → compiler → formatter bootstrap chain so RBQN can compile and execute BQN code. Runtime0 has 2 known failing tests (+´ returns wrong result, +¨ type error) that cascade into runtime1/compiler failures. This phase delivers a working compiler that can evaluate BQN expressions end-to-end.

</domain>

<decisions>
## Implementation Decisions

### Debugging strategy
- Full pipeline trace — systematically trace runtime0 execution to find all issues, not just the 2 known failures
- No instrumentation in source — debug externally using test assertions and comparisons, keep the codebase clean
- Incremental commits — commit each fix individually as it's verified, not batched

### Verification depth
- Runtime1: smoke test a subset of key functions (arithmetic, array ops, modifiers) then move to compiler — compiler tests will catch remaining issues
- Compiler: comprehensive testing — test all syntax forms including functions, trains, namespaces, headers, not just `1+1`
- Test format: both Rust integration tests (#[test]) for internal correctness AND BQN script tests for end-to-end behavior
- Failure policy: core functionality must pass to complete Phase 1, edge cases can be tracked and deferred to Phase 4 (Primitive Correctness)

### Claude's Discretion
- Whether to use CBQN as reference oracle or spec-based verification at each stage
- Formatter priority and approach — roadmap lists it but it may be blocked on deeper issues
- Error reporting style when bootstrap stages fail

</decisions>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-fix-bootstrap-pipeline*
*Context gathered: 2026-02-21*
