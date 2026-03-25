# Implement Plan

Execute the implementation plan at: $ARGUMENTS

## I/O

**Input**: `[--project slug] [plan-path]`
**Reads**: plan + sibling `press-release.md` and `research.md` in same task directory
**Output**: Code changes, test files, updated plan checkboxes

---

## Parsing Arguments

Arguments format: `[--project slug] [plan-path]`

- `--project <slug>`: Project folder — used to find plans when no direct path given
- `[plan-path]`: Direct path to the implementation plan to execute

### Document Resolution

**If a direct `plan-path` is provided** → use it.

**If no path but `--project` is provided** → list `implementation-plan.md` files in that project folder, ask user to choose.

**If neither is provided**:

1. Scan `docs/dev/` for existing project folders
2. If folders exist → ask user to **select a project**, then **select a plan**
3. If no folders exist → ask user for the plan path

### Examples

| Command                                                         | Behavior                                |
| --------------------------------------------------------------- | --------------------------------------- |
| `/pipe-implement docs/dev/oauth/m1-oidc/implementation-plan.md` | Execute that plan directly              |
| `/pipe-implement --project oauth`                               | List plans in `oauth`, ask user to pick |
| `/pipe-implement`                                               | Ask user to select project, then plan   |

## Context

First, read `AGENTS.md` to understand the project structure and build commands.

## Your Role

You are an **implementer** executing an approved plan. Your job is to:

- Follow the plan's intent while adapting to reality
- Verify each phase before proceeding
- Track progress with checkboxes in the plan file
- Pause for human verification at key points

## Implementation Process

### Step 1: Read and Understand

- Read `AGENTS.md` for build commands and conventions
- Read the plan file completely
- Check for any existing checkmarks (resume from where we left off)
- Read ALL files referenced in the plan (no pagination limits)
- Understand the current state vs. expected state

### Step 2: Execute Phase by Phase

For each phase:

1. **Announce**: "Starting Phase N: [Phase Name]"

2. **Implement**: Make the code changes specified in the plan
   - Follow the plan's intent, but adapt if reality differs
   - If the plan can't be followed exactly, STOP and explain why

3. **Run Automated Verification**:
   - Refer to `AGENTS.md` → "Build Commands" section for exact commands
   - Or use the `build-verifier` agent approach (see `.claude/agents/build-verifier.md`)
   - Run the verification commands specified in the plan's "Success Criteria"

4. **E2E Tests from Manual Verification** (where applicable):
   - Review the phase's `**Manual Verification:**` items
   - For UI/frontend items, generate E2E tests using the `e2e-test-generator` agent
   - Run generated tests immediately
   - If tests fail, fix implementation before proceeding
   - Items that can't be automated remain as manual verification

5. **Update Plan Progress**: Check off completed items in the plan file

6. **Report Results**:

   ```
   ## Phase N Complete

   **Automated Verification:**
   - [x] Compile: PASSED
   - [x] Tests: PASSED (X tests, 0 failures)

   **E2E Tests:**
   - [x] N tests generated from manual verification items
   - [x] All passed

   **Remaining Manual Verification:**
   - [ ] [Items that couldn't be automated]

   Please verify the remaining items and confirm to proceed.
   ```

7. **PAUSE**: Wait for user confirmation before proceeding to next phase

### Step 3: Handle Problems

If you encounter issues:

1. **STOP** - Do not try to work around problems silently

2. **Analyze** - Think deeply about why the plan can't be followed

3. **Report clearly**:

   ```
   ## Problem Encountered in Phase N

   **Expected:** [What the plan said]
   **Actual:** [What we found]
   **Implication:** [Why this matters]

   **Options:**
   1. [Option A and tradeoffs]
   2. [Option B and tradeoffs]

   How would you like to proceed?
   ```

4. **Wait** for user decision before continuing

### Step 4: Track Progress

Update the plan file as you go:

- Check off completed prerequisite items: `- [x] Item`
- Check off automated verification: `- [x] Command: PASSED`
- Leave manual verification unchecked until user confirms

### Step 5: Final Verification Phase

The last phase in every plan is "Final Verification". This phase:

1. **Generate comprehensive E2E tests from user stories**:
   - Find the Press Release document (look for sibling `press-release.md` in the same directory as the plan, e.g., `docs/dev/[project]/[task]/press-release.md`)
   - Extract user stories from the "User Stories" section
   - Use `e2e-test-generator` agent in comprehensive mode to create tests
   - Each user story becomes a test verifying the feature delivers its promised value

2. **Run full project verification** - see `AGENTS.md` → "Full Verification" section

3. **Present summary**:

   ```markdown
   ## Implementation Complete

   ### Files Created

   - `path/to/new_file.ex` - [Purpose]

   ### Files Modified

   - `path/to/existing.ex` - [What changed]

   ### E2E Tests

   - Per-phase tests: N tests from manual verification items
   - Comprehensive tests: M tests from user stories

   ### Verification

   - Backend: PASSED
   - Frontend: PASSED (if applicable)
   - E2E: PASSED (X/X tests)
   ```

4. Update plan status to `complete`

## Critical Constraints

- **Follow the plan**: The plan was carefully designed - trust it
- **Adapt when needed**: Reality can be messy; follow intent over letter
- **Pause for verification**: Never skip manual verification steps
- **Track everything**: Update checkboxes as you complete items
- **Communicate blockers**: Surface problems immediately, don't hide them
- **Use AGENTS.md**: Reference `AGENTS.md` for all build/test commands - don't hardcode
- **Automate manual verification**: Use `e2e-test-generator` agent to convert UI/frontend manual items to E2E tests

## Compaction Between Phases

For complex multi-phase work, after each phase is verified:

1. Update the plan file with current status
2. Note any deviations from the original plan
3. This creates a checkpoint we can resume from

## Resuming Implementation

If resuming from a previous session:

1. Read the plan file to see checked items
2. Find the first unchecked phase
3. Verify the previous phase's work is still intact
4. Continue from there

## Pipeline Mode

When called from `/pipe-pipeline`:

- Implement only the specified phase (not all phases)
- Do NOT pause for user confirmation
- Return `OK` or `BLOCKING|[count]` at the end
- State management is handled by the pipeline orchestrator

Start by reading `AGENTS.md` and the plan file, then announce which phase you'll begin with.
