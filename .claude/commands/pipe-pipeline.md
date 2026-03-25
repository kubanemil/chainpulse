# Pipeline Orchestrator

Orchestrate the full development pipeline from Press Release to implementation: $ARGUMENTS

```
PRESS_RELEASE → PRESS_RELEASE_REVIEW → RESEARCH → RESEARCH_REVIEW → PLAN → PLAN_REVIEW → [CHECKPOINT] → IMPLEMENT
              ↑______________________|           ↑________________|        ↑_____________|                  │
              (revision loop)                    (revision loop)           (revision loop)                  ├→ Per-phase: code + E2E from manual verification
                                                                                                            └→ Final phase: comprehensive E2E from user stories
```

---

## Core Design Principles

**1. Agent-Owned State**: Background agents manage ALL state in `.pipeline/[ref]-state.json`. The main thread is a minimal dispatcher that only reads state and spawns agents.

**2. Ultra-Minimal Returns**: Agents return ONLY `OK` or `BLOCKING|N`. No summaries, no file paths in returns—agents write everything to state/result files.

**3. Checkpoint Before Implementation**: After PLAN_REVIEW passes, pipeline pauses for user to `/clear` context before heavy implementation work.

**Why?** Main thread stays under ~200 tokens for the entire pipeline. State survives `/clear` and autocompact.

---

## I/O

**Input**: `[--project slug] [--ref id] [feature-description]`
**Output**: `docs/dev/[project]/[task-slug]/press-release.md`, `research.md`, `implementation-plan.md` + code changes
**State**: `.pipeline/[project-slug]-[reference]-state.json`

---

## Arguments

Format: `[--project slug] [--ref id] [feature-description]`

- `--project <slug>`: Project folder name under `docs/dev/`
- `--ref <id>`: Milestone (`m0.5`), ticket (`PROJ-123`), or issue (`#42`)
- `[feature-description]`: Plain text — everything after flags is the description

### Project Resolution

If `--project` is provided → use it as the project slug.

If `--project` is omitted:

1. Scan `docs/dev/` for existing project folders
2. If folders exist → ask user to **select from the list** OR **type a new slug**
3. If no folders exist → ask user to **type a project slug**

### Task Slug

Derived from reference + description: `[ref-lowercase]-[description-kebab-case]`

### Examples

| Command                                                     | Project      | Task Slug          | State File                            |
| ----------------------------------------------------------- | ------------ | ------------------ | ------------------------------------- |
| `/pipe-pipeline --project workforce --ref m0.5 tenant list` | `workforce`  | `m0.5-tenant-list` | `.pipeline/workforce-m0.5-state.json` |
| `/pipe-pipeline --ref m1 oidc` (projects exist)             | user selects | `m1-oidc`          | `.pipeline/[selected]-m1-state.json`  |

---

## State File

Location: `.pipeline/[project-slug]-[reference]-state.json`

```json
{
  "reference": "m0.5",
  "project": "workforce-platform",
  "feature": "tenant-list-and-status",
  "task_slug": "m0.5-tenant-list",
  "started_at": "2025-01-10T10:00:00Z",
  "current_stage": "IMPLEMENT",
  "current_phase": 2,
  "total_phases": 5,
  "files": {
    "press_release": "docs/dev/workforce-platform/m0.5-tenant-list/press-release.md",
    "research": "docs/dev/workforce-platform/m0.5-tenant-list/research.md",
    "plan": "docs/dev/workforce-platform/m0.5-tenant-list/implementation-plan.md"
  },
  "stages": {
    "PRESS_RELEASE": {
      "status": "complete",
      "result_file": ".pipeline/workforce-platform-m0.5-press-release-result.md"
    },
    "PRESS_RELEASE_REVIEW": {
      "status": "complete",
      "result_file": ".pipeline/workforce-platform-m0.5-press-release-review-result.md"
    },
    "RESEARCH": {
      "status": "complete",
      "result_file": ".pipeline/workforce-platform-m0.5-research-result.md"
    },
    "RESEARCH_REVIEW": {
      "status": "complete",
      "result_file": ".pipeline/workforce-platform-m0.5-research-review-result.md"
    },
    "PLAN": {
      "status": "complete",
      "result_file": ".pipeline/workforce-platform-m0.5-plan-result.md"
    },
    "PLAN_REVIEW": {
      "status": "complete",
      "result_file": ".pipeline/workforce-platform-m0.5-plan-review-result.md"
    },
    "IMPLEMENT": {
      "status": "in_progress",
      "phases": [
        { "phase": 1, "name": "Database schema", "status": "complete" },
        { "phase": 2, "name": "API endpoints", "status": "in_progress" },
        { "phase": 3, "name": "Frontend components", "status": "pending" },
        { "phase": 4, "name": "Final Verification", "status": "pending" }
      ]
    },
    "agent_ids": {
      "press_release": "abc123",
      "research": "def456"
    }
  }
}
```

Status values: `pending`, `in_progress`, `complete`, `needs_revision`, `failed`, `blocked`

---

## Stage Agent Template

All stage agents follow this pattern:

```
You are running as part of an automated pipeline.

STATE FILE: .pipeline/[project-slug]-[reference]-state.json

BEFORE STARTING:
1. Read state file
2. Update state: set stages.[STAGE].status = "in_progress", record agent_id
3. Write state file

DO THE WORK:
4. Read the command file from .claude/commands/[command].md
5. Follow those instructions to complete the stage (pass --project [project-slug] to commands)
6. Write output to designated location under docs/dev/[project-slug]/[task-slug]/
7. Write summary to: .pipeline/[project-slug]-[reference]-[stage]-result.md

BEFORE RETURNING:
8. Update state: stages.[STAGE].status = "complete", record result_file path
9. Update state: current_stage = [NEXT_STAGE]
10. Write state file

RETURN:
- Success: OK
- Review found blocking issues: BLOCKING|[count]

Do NOT include any other text.
```

---

## Stage Details

| Stage                | Command File                                    | Primary Output                                   | Next Stage           |
| -------------------- | ----------------------------------------------- | ------------------------------------------------ | -------------------- |
| PRESS_RELEASE        | `.claude/commands/pipe-press-release.md`        | docs/dev/[project]/[task]/press-release.md       | PRESS_RELEASE_REVIEW |
| PRESS_RELEASE_REVIEW | `.claude/commands/pipe-review-press-release.md` | —                                                | RESEARCH\*           |
| RESEARCH             | `.claude/commands/pipe-research.md`             | docs/dev/[project]/[task]/research.md            | RESEARCH_REVIEW      |
| RESEARCH_REVIEW      | `.claude/commands/pipe-review-research.md`      | —                                                | PLAN\*               |
| PLAN                 | `.claude/commands/pipe-plan-implementation.md`  | docs/dev/[project]/[task]/implementation-plan.md | PLAN_REVIEW          |
| PLAN_REVIEW          | `.claude/commands/pipe-review-plan.md`          | —                                                | CHECKPOINT\*         |
| IMPLEMENT            | `.claude/commands/pipe-implement.md`            | (code changes)                                   | COMPLETE             |

\*Review stages: On `BLOCKING`, set `status = "needs_revision"` and `blocking_count`. On success, advance to next stage.

### Stage-Specific Notes

**PRESS_RELEASE**: Read command file, run autonomously (no user interaction). Record output path in `files.press_release`.

**RESEARCH**: Read Press Release from `files.press_release` for context. Record output path in `files.research`.

**PLAN**: Read Press Release and research for context. Record output path in `files.plan`.

**Reviews**: Count blocking issues (❌ items). If blocking issues exist, return `BLOCKING|[count]`.

---

## Checkpoint Protocol

After PLAN_REVIEW succeeds, the pipeline pauses:

```
✓ PRESS_RELEASE: OK
✓ PRESS_RELEASE_REVIEW: OK
✓ RESEARCH: OK
✓ RESEARCH_REVIEW: OK
✓ PLAN: OK
✓ PLAN_REVIEW: OK

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
CHECKPOINT: Ready for implementation

Run `/clear` then `/pipe-pipeline` to continue with fresh context.
Or reply "continue" to proceed without clearing.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

When resumed, pipeline reads state, sees `current_stage: "CHECKPOINT"`, moves to IMPLEMENT.

---

## Implementation Phases

Each phase runs in a **fresh background agent session**.

### Phase Agent Prompt

```
You are running as part of an automated pipeline.

STATE FILE: .pipeline/[project-slug]-[reference]-state.json

1. Read state file to get: plan path, current_phase, press release path and research path
2. Update state: stages.IMPLEMENT.phases[current_phase].status = "in_progress"
3. Read and follow `.claude/commands/pipe-implement.md` for Phase [current_phase] ONLY
   - Do NOT implement other phases
   - Do NOT pause for user confirmation (pipeline mode)
4. Write results to: .pipeline/[project-slug]-[reference]-phase-[X]-result.md
5. Update state: phases[current_phase].status = "complete", increment current_phase
6. If last phase: stages.IMPLEMENT.status = "complete"

RETURN: OK or BLOCKING|[count]
```

The `pipe-implement.md` command handles all implementation logic including E2E test generation.

---

## Revision Protocol

When a review returns `BLOCKING|N`:

1. Read the result file for blocking issues
2. Present to user:

```
## Review Found Blocking Issues

Stage: [STAGE]
Document: [file-path]

### Blocking Issues
1. [Issue from review]
2. [Issue from review]

### Options
1. **Auto-revise** - Resume previous agent with findings (recommended)
2. **Manual fix** - Pause for you to edit, then continue
3. **Override** - Proceed despite issues (not recommended)
```

3. **Auto-revise**: Resume the previous stage's agent (using stored agent_id), pass review findings, re-run review
4. **Manual fix**: Wait for "continue", re-run review
5. **Override**: Log override, proceed to next stage

---

## Main Dispatcher Loop

```
1. Check for existing state file
   - If exists: Display progress, ask resume or fresh start
   - If fresh: Create state with current_stage = "PRESS_RELEASE"

2. Loop:
   - Read state → get current_stage
   - If COMPLETE: Print "✓ Pipeline complete", exit
   - If CHECKPOINT: Print checkpoint message, exit (wait for user)
   - Spawn background agent for current_stage
   - Receive response (OK or BLOCKING|N)
   - If OK: Print "✓ [STAGE]: OK", continue loop
   - If BLOCKING: Read result file, handle revision, exit
```

### Example Output

```
Pipeline: workforce-platform / m0.5 tenant-list-and-status

→ PRESS_RELEASE...
✓ PRESS_RELEASE: OK
→ PRESS_RELEASE_REVIEW...
✓ PRESS_RELEASE_REVIEW: OK
→ RESEARCH...
✓ RESEARCH: OK
→ RESEARCH_REVIEW...
✓ RESEARCH_REVIEW: OK
→ PLAN...
✓ PLAN: OK
→ PLAN_REVIEW...
✓ PLAN_REVIEW: OK

[CHECKPOINT MESSAGE]

> /clear
> /pipe-pipeline

Resuming from: IMPLEMENT (Phase 1/4)

→ Phase 1: Database schema...
✓ Phase 1: OK
→ Phase 2: API endpoints...
✓ Phase 2: OK
→ Phase 3: Frontend components...
  → E2E from manual verification: 4/4 passed
✓ Phase 3: OK
→ Phase 4: Final Verification...
  → Comprehensive E2E from user stories: 8/8 passed
  → Full verification: PASSED
✓ Phase 4: OK

✓ Pipeline complete. See .pipeline/workforce-platform-m0.5-final-result.md
```

---

## Recovery

If context is lost (autocompact, session restart), running `/pipe-pipeline` detects existing state:

```
## Pipeline Recovery

**Reference**: m0.5
**Current Stage**: IMPLEMENT (Phase 2/4)

**Completed**: PRESS_RELEASE ✓, PRESS_RELEASE_REVIEW ✓, RESEARCH ✓, RESEARCH_REVIEW ✓, PLAN ✓, PLAN_REVIEW ✓, Phase 1 ✓
**In Progress**: Phase 2

Resume from Phase 2?
```

---

## Error Handling

| Scenario      | Action                                                                           |
| ------------- | -------------------------------------------------------------------------------- |
| Agent timeout | Check output file for partial results, ask user to wait or abort                 |
| Agent error   | Display error, ask retry or abort                                                |
| Phase failure | Present options: retry phase, manual intervention, skip (not recommended), abort |

### Phase Failure Options

```
## Phase [X] Issue: [Name]

**Problem**: [Description]
**Current state**: [Files created/modified, test status]

**Options**:
1. **Retry phase** - Fresh agent, same phase
2. **Manual intervention** - Pause for you to fix
3. **Skip phase** - Not recommended
4. **Abort pipeline** - Keep current progress
```

---

## Completion Summary

```
## Pipeline Complete

**Reference**: m0.5 | **Feature**: Tenant List and Status

### Documents
- Press Release: docs/dev/workforce-platform/m0.5-tenant-list/press-release.md
- Research: docs/dev/workforce-platform/m0.5-tenant-list/research.md
- Plan: docs/dev/workforce-platform/m0.5-tenant-list/implementation-plan.md

### Implementation
| Phase | Name | Status |
|-------|------|--------|
| 1 | Database schema | ✓ |
| 2 | API endpoints | ✓ |
| 3 | Frontend components | ✓ |
| 4 | Final Verification | ✓ |

### E2E Tests
- Per-phase tests: 4 tests passed
- Comprehensive tests: 8 tests passed (covering all user stories)

### Next Steps
1. Review changes manually
2. Test end-to-end
3. Create PR
```

---

## Notes

- Add `.pipeline/` to `.gitignore`
- For complex/ambiguous features, consider running stages manually (`/pipe-press-release`, `/pipe-research`, `/pipe-plan-implementation`, `/pipe-implement`)
- Pipeline works best with well-understood features in codebases with established patterns

---

Start by parsing arguments and checking for existing state file.
