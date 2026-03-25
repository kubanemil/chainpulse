# Generate E2E Tests

Generate Playwright E2E tests from implementation plan or press release: $ARGUMENTS

## I/O

**Input**: `[--project slug] [plan-path] [--phase N | --comprehensive]`
**Reads**: plan + sibling `press-release.md` (for comprehensive mode)
**Output**: E2E test files in the project's test directory

---

## Parsing Arguments

Arguments format: `[--project slug] [plan-path] [--phase N] [--comprehensive]`

- `--project <slug>`: Project folder — used to find plans when no direct path given
- `[plan-path]`: Direct path to the plan file
- `--phase <N>`: Generate tests for a specific phase's manual verification items
- `--comprehensive`: Generate tests from PRFAQ user stories (for final verification)

Must specify either `--phase N` or `--comprehensive` (not both).

### Document Resolution

**If a direct `plan-path` is provided** → use it.

**If no path but `--project` is provided** → list `implementation-plan.md` files in that project folder, ask user to choose.

**If neither is provided**:

1. Scan `docs/dev/` for existing project folders
2. If folders exist → ask user to **select a project**, then **select a plan**
3. If no folders exist → ask user for the plan path

### Examples

| Command                                                                                | Behavior                             |
| -------------------------------------------------------------------------------------- | ------------------------------------ |
| `/pipe-e2e docs/dev/my-project/m10-enum-fields/implementation-plan.md --phase 3`       | Generate phase 3 tests               |
| `/pipe-e2e docs/dev/my-project/m10-enum-fields/implementation-plan.md --comprehensive` | Generate full user story tests       |
| `/pipe-e2e --project my-project --comprehensive`                                       | Ask user to pick plan, then generate |

---

## Your Role

You orchestrate E2E test generation by:

1. Parsing arguments to determine mode
2. Spawning the `e2e-test-generator` agent with the right context
3. Reporting results

---

## Step 1: Parse Arguments

Determine the mode:

- If `--phase N` → **Per-phase mode**: Generate tests from that phase's manual verification items
- If `--comprehensive` → **Comprehensive mode**: Generate tests from PRFAQ user stories
- If neither → Error: must specify a mode

Validate:

- Plan path must exist
- Phase number (if provided) must exist in the plan

---

## Step 2: Spawn Agent

**For per-phase mode:**

```
Read .claude/agents/e2e-test-generator.md and follow its instructions.

MODE: Per-phase
PLAN FILE: [plan-path]
PHASE: [N]

Generate E2E tests from Phase [N]'s manual verification items.
```

**For comprehensive mode:**

```
Read .claude/agents/e2e-test-generator.md and follow its instructions.

MODE: Comprehensive
PLAN FILE: [plan-path]

Generate E2E tests from the PRFAQ user stories.
Find the press release as a sibling file in the same directory as the plan (press-release.md).
```

---

## Step 3: Report Results

Present the agent's output:

- Tests generated and their sources
- Pass/fail status
- Items not covered

---

## When This Command Is Used

1. **During implementation** - Called by `/pipe-implement` after each phase to automate manual verification items
2. **Standalone** - To add E2E tests to an existing codebase or regenerate tests

---

Start by parsing arguments from `$ARGUMENTS`.
