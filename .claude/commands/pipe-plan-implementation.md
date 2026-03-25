# Create Implementation Plan

Create a detailed implementation plan for: $ARGUMENTS

## I/O

**Input**: `[--project slug] [--ref id] [--research path] [feature-description]`
**Reads**: `docs/dev/[project-slug]/[task-slug]/research.md`, `press-release.md` (siblings, if exist)
**Output**: `docs/dev/[project-slug]/[task-slug]/implementation-plan.md`
**Commit**:
```
[project-slug/task-slug] implementation-plan: <short summary>

<description of phases, architecture decisions, and approach>
```

---

## Parsing Arguments

Arguments format: `[--project slug] [--ref id] [--research path] [feature-description]`

- `--project <slug>`: Project folder name under `docs/dev/`
- `--ref <id>`: Optional. Milestone (`m1`, `M0.3`), ticket (`PROJ-123`), or issue (`#42`)
- `--research <path>`: Path to research document to read first
- `[feature-description]`: Plain text — everything after flags is the description

### Project Resolution

If `--project` is provided → use it as the project slug.

If `--project` is omitted:

1. Scan `docs/dev/` for existing project folders
2. If folders exist → ask user to **select from the list** OR **type a new slug**
3. If no folders exist → ask user to **type a project slug**

Create `docs/dev/[project-slug]/` if it doesn't exist.

### Task Slug

The task slug identifies the task folder within the project:

- With reference: `[ref-lowercase]-[feature-kebab-case]` (e.g., `m1-tenant-creation`)
- Without reference: `[feature-kebab-case]` (e.g., `add-user-roles`)

Rules: lowercase, kebab-case, max 4 words for description.

If `--research` is provided, read that document FIRST before any other context gathering.

### Examples

| Command                                                                                                       | Project      | Task Slug            | Output                                                         |
| ------------------------------------------------------------------------------------------------------------- | ------------ | -------------------- | -------------------------------------------------------------- |
| `/pipe-plan-implementation --project workforce --ref m1 tenant creation`                                      | `workforce`  | `m1-tenant-creation` | `docs/dev/workforce/m1-tenant-creation/implementation-plan.md` |
| `/pipe-plan-implementation --project workforce --research docs/dev/workforce/add-roles/research.md add roles` | `workforce`  | `add-roles`          | `docs/dev/workforce/add-roles/implementation-plan.md`          |
| `/pipe-plan-implementation --ref PROJ-123 billing` (projects exist)                                           | user selects | `proj-123-billing`   | `docs/dev/[selected]/proj-123-billing/implementation-plan.md`  |

## Context

First, read `AGENTS.md` to understand the project structure, conventions, and build commands.

## Your Role

You are a **technical architect** creating an executable specification. The plan must be:

- Precise enough that implementation requires no guesswork
- Phased with clear verification at each step
- Complete with all decisions made upfront
- Concise and focused on structure, contracts, and flow - NOT implementation code (target below 600 lines)

**Plan Should NOT Include**:

- Full function implementations
- Complete file contents
- Test file implementations
- Boilerplate code
- Code that will be written again during implementation

**Why**: The implemented code is the source of truth. Reviewing code twice (in plan, then in implementation) is redundant.

## Planning Process

### Step 1: Context Gathering

- Read `AGENTS.md` for project conventions
- Read ALL files provided by the user completely
- If a research document exists (via `--research`), read it first
- Spawn parallel exploration agents to understand:
  - Current codebase patterns and conventions
  - Existing similar implementations
  - Test patterns used in the project
- Present your understanding and ask clarifying questions

**Use these specialized agents** (definitions in `.claude/agents/`):

- `codebase-locator` - Find relevant files
- `codebase-analyzer` - Understand how existing code works
- `codebase-pattern-finder` - Find patterns to follow
- `docs-locator` - Find related documentation

### Step 2: Research & Discovery

- If the user corrects your understanding, DO NOT just accept it
- Spawn new research tasks to verify against actual code
- Track what you've researched vs. what remains unknown
- Present findings with multiple design options if applicable

### Step 3: Plan Structure

Before writing details, propose the phase structure:

- How many phases?
- What does each phase accomplish?
- What are the dependencies between phases?

Get user approval on structure before proceeding.

### Step 4: Detailed Plan Writing

Create a markdown file at: `docs/dev/[project-slug]/[task-slug]/implementation-plan.md`

**Task slug format**:

- With reference: `[reference]-[feature-slug]` (e.g., `m0.3-tenant-creation`, `proj-123-user-auth`)
- Without reference: `[feature-slug]` (e.g., `add-user-roles`)

Rules:

- Convert reference to lowercase
- Use kebab-case for the feature slug
- The file is always named `implementation-plan.md` — the task slug is the folder name
- Create the `docs/dev/[project-slug]/[task-slug]/` directory if it doesn't exist

Use this structure:

````markdown
---
date: [ISO date]
author: [model name from session]
status: draft | approved | in-progress | complete
branch: [target branch]
related_research: [path to research doc if exists]
---

# Plan: [Feature/Task Name]

## Overview

[1-2 paragraph summary of what this plan accomplishes]

## Architecture Decisions

[Bullet list of key decisions made and why]

- Decision 1: [choice] because [rationale]
- Decision 2: [choice] because [rationale]

## Prerequisites

- [ ] [Any setup or preparation needed]

---

## Phase 1: [Phase Name]

### Goal

[What this phase accomplishes]

### Files to Modify

- `path/to/file` - [What changes]
- `path/to/new_file` - [New file: purpose]

### Module Design

**`ModuleName`** - [One sentence describing responsibility]

Key functions:

- `function_name(param1, param2) -> return_type` - [Purpose]
- `other_function(input) -> success | error` - [Purpose]

### Data Structures

```
# Schema/struct fields only - no full implementation
ModuleName {
  field1: type
  field2: type
}
```
````

### Integration

- [How this connects to existing code]
- [What calls what, data flow]

### Success Criteria

**Automated Verification:**
See `AGENTS.md` for exact commands. Run the appropriate verification commands for the areas changed.

**Manual Verification:**
These items become E2E test scenarios during implementation (where applicable).

- [ ] [Behavior to verify]
- [ ] [Expected output to check]

---

## Phase 2: [Phase Name]

[Same structure as Phase 1]

---

## Phase N: Final Verification

### Goal

Ensure all components work together end-to-end and satisfy the user stories from the Press Release.

### Success Criteria

**Automated Verification:**

Run full project verification (see `AGENTS.md` → "Full Verification")

**Comprehensive E2E Tests:**

Generate E2E tests from the Press Release user stories. Each user story becomes a test scenario verifying the feature delivers its promised value.

**Manual Verification:**

- [ ] [End-to-end test scenario]
- [ ] [Edge case to verify]
- [ ] [Any remaining edge cases not covered by E2E]

---

## Rollback Plan

[How to undo changes if something goes wrong - brief]

## Open Questions

[Any decisions deferred or questions for future consideration]

````

### Step 5: Review and Iterate

- Present the plan to the user
- Incorporate feedback
- Update the plan file
- Repeat until user approves

### Step 6: Finalize

- Update the plan status to `approved`
- Commit following the format in the `## I/O` section at the top of this file

## Critical Constraints

- **No implementation code**: Describe what to build, not how to code it
- **No ambiguity**: Every decision must be made before finalizing
- **Specific file paths**: List exact files to create/modify
- **Testable criteria**: Each phase must have runnable verification
- **Atomic phases**: Each phase should be independently verifiable
- **Clear dependencies**: Note which phases depend on others
- **Reference AGENTS.md**: Don't duplicate build commands - reference `AGENTS.md`
- **Concise**: If your plan exceeds 600 lines, you're including too much detail

## Questions to Resolve Before Finalizing

Ensure the plan addresses:

1. What new files need to be created?
2. What existing files need modification?
3. What tests need to be written?
4. How do we verify each phase works?
5. How do components integrate?
6. What could go wrong and how do we handle it?

## Handoff to Implementation

**IMPORTANT**: At the end of EVERY response where you present a plan (draft or revised), you MUST include a "Next Steps" section:

```markdown
---

## Next Steps

If you approve this plan, proceed to implementation:

/pipe-implement docs/dev/[project-slug]/[task-slug]/implementation-plan.md

If you want changes:

- Tell me what to revise and I'll update the plan
- Ask questions about specific phases or decisions
````

Replace `[project-slug]` and `[task-slug]` with the actual values (e.g., `workforce-platform/m0.3-tenant-creation`).

When the user approves the plan:

1. Update the plan status to `approved` in the YAML frontmatter
2. Commit the plan file
3. Present the `/pipe-implement` command one final time

Start by gathering context and asking clarifying questions.
