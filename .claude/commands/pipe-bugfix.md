# Bug Fix

Fix a bug with targeted research, planning, implementation, and testing: $ARGUMENTS

```
/pipe-bugfix [--ref id] [description]
    │
    ├─→ Research (reuses /pipe-research with bug-fix constraints)
    ├─→ Scope check: Is this really a bug?
    ├─→ Plan (reuses /pipe-plan-implementation with bug-fix constraints)
    ├─→ CHECKPOINT: Present findings, get approval
    └─→ Implement (reuses /pipe-implement) → Commit
```

---

## Parsing Arguments

Arguments format: `[--ref id] [bug description]`

- `--ref <id>`: Optional. Milestone (`m0.6`), ticket (`PROJ-123`), or issue (`#123`)
- `[bug description]`: Plain text — everything after flags is the description

**Task slug**: Synthesized with a `bugfix-` prefix:

- With `--ref`: `bugfix-[ref]-[description-slug]` (e.g., `bugfix-m0.6-tenant-suspension`)
- Without `--ref`: `bugfix-[description-slug]` (e.g., `bugfix-negative-user-count`)

Examples:

- `/pipe-bugfix --ref m0.6 tenant suspension doesn't block API access` → `docs/dev/bugfix-m0.6-tenant-suspension/`
- `/pipe-bugfix --ref 123 login fails after password reset` → `docs/dev/bugfix-123-login-fails/`
- `/pipe-bugfix user count shows negative numbers` → `docs/dev/bugfix-negative-user-count/`

---

## Your Role

You are a **bug fix orchestrator**. You reuse existing commands (`/pipe-research`, `/pipe-plan-implementation`, `/pipe-implement`) but constrain them for bug-fix scope.

You are NOT:

- Adding features
- Refactoring unrelated code
- Improving architecture
- Fixing other bugs you notice

---

## Phase 1: Bug-Focused Research

Spawn a background agent that follows `.claude/commands/pipe-research.md` with these additional constraints:

**Agent prompt**:

```
Read and follow the instructions in .claude/commands/pipe-research.md

RESEARCH TOPIC: [bug description]
REFERENCE: [ref or "none"]

ADDITIONAL CONSTRAINTS FOR BUG FIX:
- Focus ONLY on code paths related to this specific bug
- Identify the root cause (or top 2-3 hypotheses if uncertain)
- Check existing test coverage for the affected code
- Find similar working code that handles this correctly
- Do NOT explore unrelated areas of the codebase
- Keep research document concise (under 200 lines)

ADDITIONAL SECTIONS TO INCLUDE:
## Root Cause Analysis
[Most likely cause with file:line evidence]

## Test Coverage Gap
[What's not tested that should be]

## Similar Working Code
[Patterns from code that handles this correctly]

Save to: docs/dev/[task-slug]/research.md (e.g., docs/dev/bugfix-m0.6-tenant-suspension/research.md)
```

Wait for research to complete.

---

## Phase 2: Scope Check

Review the research findings and evaluate scope:

**Escalation triggers** (report and ask user):

- More than 3-4 files need significant changes
- Requires database migration
- Would change public API contract
- No clear single root cause (architectural issue)
- Actually a missing feature, not broken behavior

**If escalation triggered**:

```markdown
## Scope Assessment

This appears larger than a typical bug fix:

**Findings**: [summary from research]

**Concern**: [why this might not be a simple bug]

**Options**:

1. **Proceed as bug fix** - Make minimal changes to address immediate symptom
2. **Escalate to pipeline** - Run `/pipe-pipeline --ref [ref] [description]` for proper feature work
3. **Abort** - Need more information before proceeding

Which would you like?
```

Wait for user decision before proceeding.

**If scope is appropriate**, continue to Phase 3.

---

## Phase 3: Bug-Focused Planning

Spawn a background agent that follows `.claude/commands/pipe-plan-implementation.md` with these additional constraints:

**Agent prompt**:

```
Read and follow the instructions in .claude/commands/pipe-plan-implementation.md

FEATURE: Bug fix - [description]
REFERENCE: [ref or "none"]
RESEARCH: docs/dev/[task-slug]/research.md

ADDITIONAL CONSTRAINTS FOR BUG FIX:
- Minimal changes ONLY - fix the bug, nothing else
- MUST include a regression test (mandatory, not optional)
- Single phase only (no multi-phase plans for bug fixes)
- Plan should be under 150 lines
- Do NOT refactor surrounding code
- Do NOT add features
- Do NOT improve error messages unless that IS the bug

REQUIRED SECTIONS:
## Fix Approach
[What to change and why - brief]

## Regression Test
- File: [test file path]
- Test name: test [description]
- Scenario: [what the test verifies - should fail before fix, pass after]

## Verification
[How to verify the fix works]

Save to: docs/dev/[task-slug]/implementation-plan.md (e.g., docs/dev/bugfix-m0.6-tenant-suspension/implementation-plan.md)

SKIP THESE SECTIONS (not needed for bug fixes):
- Rollback Plan (changes are minimal)
- Multiple phases (single phase only)
- Open Questions (should be resolved in research)
```

Wait for plan to complete.

---

## Phase 4: Approval Checkpoint

Present a summary to the user:

```markdown
## Bug Fix Ready: [Brief Title]

**Reference**: [ref or "none"]
**Bug**: [user's original description]

---

### Root Cause (from research)

[Brief summary with file:line reference]

---

### Fix Plan

**Changes**:

- `path/to/file.ex` - [what changes]

**Regression Test**:

- `path/to/test.exs` - [test scenario]

---

### Documents Created

- Research: `docs/dev/[task-slug]/research.md`
- Plan: `docs/dev/[task-slug]/implementation-plan.md`

---

**Approve to implement?**

Reply "proceed" to implement, or ask questions / request changes.
```

**BLOCKING**: Do not implement until user approves.

---

## Phase 5: Implementation

On approval, spawn a background agent that follows `.claude/commands/pipe-implement.md`:

**Agent prompt**:

```
Read and follow the instructions in .claude/commands/pipe-implement.md

PLAN: docs/dev/[task-slug]/implementation-plan.md

ADDITIONAL CONSTRAINTS FOR BUG FIX:
- Implement ONLY what's in the plan
- The regression test is MANDATORY - do not skip it
- Run tests after implementation
- Do NOT make additional improvements
- Do NOT fix other issues you notice
- Keep changes minimal

VERIFICATION:
- Run the new regression test
- Run related existing tests
- Run full test suite for affected area
```

Wait for implementation to complete.

---

## Phase 6: Verification & Commit

**If implementation succeeds and tests pass**:

```markdown
## Bug Fix Complete

### Changes Made

- `path/to/file.ex` - [what changed]

### Test Added

- `path/to/test.exs` - [test description]

### Verification

- [x] New regression test passes
- [x] Existing tests pass
- [x] Build succeeds

Creating commit...
```

Create commit with conventional commit format:

```bash
git add [changed files]
git commit -m "$(cat <<'EOF'
fix: [brief description]

[Root cause - 1-2 sentences from research]

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

**If tests fail**:

```markdown
## Verification Failed

**Issue**: [what failed]
```

[error output]

```

**Options**:
1. **Debug** - I'll investigate the failure
2. **Abort** - Revert changes and stop
3. **Manual** - You fix the remaining issue

Which would you like?
```

Do NOT commit if tests fail.

---

## Commit Message Format

Use conventional commits:

```
fix: [brief description]

[Root cause explanation - 1-2 sentences]

Co-Authored-By: Claude <noreply@anthropic.com>
```

---

## Files Created

Unlike the full pipeline, bug fixes create minimal documentation:

| File                                                  | Purpose                       |
| ----------------------------------------------------- | ----------------------------- |
| `docs/dev/bugfix-[ref]-[slug]/research.md`            | Root cause analysis (concise) |
| `docs/dev/bugfix-[ref]-[slug]/implementation-plan.md` | Fix plan (single phase)       |

These are shorter than normal research/plan docs since bug scope is limited.

---

## Critical Constraints

- **Reuse existing commands**: Research, plan, implement follow their respective `.claude/commands/*.md` files
- **Add constraints, don't duplicate**: Pass bug-fix constraints to existing commands
- **Minimal changes**: Fix the bug, nothing else
- **Regression test required**: Every fix must have a test
- **Single phase**: Bug fixes don't need multi-phase plans
- **Approval required**: Never implement without user approval
- **Tests must pass**: Never commit with failing tests

---

## What This Command is NOT For

- **Feature gaps**: "It should also do X" → Use `/pipe-pipeline`
- **Refactoring**: "This code is messy" → Use `/pipe-plan-implementation`
- **Multiple bugs**: Fix one at a time
- **Unclear issues**: "Something's wrong" → Need specific bug description

---

## Quick Reference

```
/pipe-bugfix m0.6 tenant suspension doesn't block API access

→ Research (via /pipe-research + constraints): Find root cause
→ Scope check: Is this a bug or feature gap?
→ Plan (via /pipe-plan-implementation + constraints): Design fix + test (single phase)
→ Present: "Here's the plan, approve?"
→ [User approves]
→ Implement (via /pipe-implement + constraints)
→ Run tests
→ Commit: "fix: tenant suspension now blocks API access"
```

---

Start by parsing arguments and spawning the research agent with bug-fix constraints.
