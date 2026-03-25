# Commands - AI Agent Guidelines

Pipeline commands invoked via `/pipe-*` that orchestrate development workflows.

> For project-wide guidelines, see [root AGENTS.md](../AGENTS.md)

## Overview

Commands are user-invocable workflows that orchestrate multi-step processes. They read the target project's `AGENTS.md` for context, spawn agents for specialized tasks, and maintain state across steps. Commands can run standalone or as part of the automated `/pipe-pipeline`.

## Directory Structure

```
commands/
├── pipe-init.md              # Initialize AGENTS.md for a project
├── pipe-discover-product.md  # Interactive product discovery
├── pipe-research.md          # Research codebase topics
├── pipe-review-research.md   # Review research documents
├── pipe-design-solution.md   # Explore technical approaches
├── pipe-project-charter.md   # Create project charters
├── pipe-project-plan.md      # Create high-level project plans
├── pipe-press-release.md     # Create PRFAQ documents
├── pipe-review-press-release.md # Review PRFAQ documents
├── pipe-plan-implementation.md  # Create implementation plans
├── pipe-review-plan.md       # Review implementation plans
├── pipe-implement.md         # Execute implementation plans
├── pipe-e2e.md               # Generate E2E tests standalone
├── pipe-bugfix.md            # Fix bugs (research → plan → fix)
└── pipe-pipeline.md          # Full automated pipeline orchestrator
```

## Design Principles

### Intentional Context Compaction

Each command in the pipeline reads only its **immediate predecessor's output**, not all upstream documents. This is by design:

- **Each document is a distillation** — the press release distills product discovery + solution design into customer outcomes. The research distills the codebase into what matters for that task. The plan distills research + press release into executable phases.
- **Passing all upstream docs would undermine this compaction** — if something from product discovery matters for the plan, it should already be captured in the press release or research that feeds into it.
- **Context budget is finite** — later stages (especially implementation) need room for code. Bloating them with upstream docs crowds out the actual work.
- **Signal over noise** — upstream docs may contain details that were intentionally scoped out. Exposing them risks the LLM picking up on deferred items.

The one exception is the **pipeline orchestrator** (`pipe-pipeline`), which is a thin dispatcher that tracks state but never reads document content.

### Flag-Based Argument Parsing

All structured values are passed as explicit flags (`--project`, `--ref`, `--research`). Free-form description text comes last. This eliminates parsing ambiguity — no pattern-matching needed to distinguish a reference from a description word.

```
/pipe-research --project oauth --ref m1 implement OIDC provider foundation
```

### Any Command as Entry Point

Every command can be the first step. If `--project` is omitted, the command asks the user to select an existing project or type a new slug. This means you can start with `/pipe-research` directly without needing `/pipe-discover-product` first.

---

## Patterns

### Command File Structure

Every command follows this structure:

```markdown
# [Command Title]

[Purpose description]: $ARGUMENTS

## Parsing Arguments

[How to parse $ARGUMENTS - reference detection, flags]

## Context

First, read `AGENTS.md` to understand...

## Your Role

You are a **[role]**. Your job is to:

- [Responsibility 1]
- [Responsibility 2]

## [Process/Workflow Section]

### Step 1: [Name]

[Instructions]

### Step 2: [Name]

[Instructions]

## Critical Constraints

- [Constraint list]

## Handoff to [Next Command]

[How to transition to the next pipeline stage]
```

### I/O Contract Pattern

Every command declares its inputs and outputs at the top:

```markdown
## I/O

**Input**: `[--project slug] [--ref id] [description]`
**Reads**: `docs/dev/[project-slug]/[task-slug]/press-release.md` (sibling, if exists)
**Output**: `docs/dev/[project-slug]/[task-slug]/research.md`
**Commit**:
\```
[project-slug/task-slug] research: <short summary>

<description of findings and key insights>
\```
```

### Argument Parsing Pattern

All flags come before free-form description. Flags use `--name value` format:

```markdown
## Parsing Arguments

Arguments format: `[--project slug] [--ref id] [description]`

- `--project <slug>`: Project folder name under `docs/dev/`
- `--ref <id>`: Optional. Milestone (`m1`), ticket (`PROJ-123`), or issue (`#123`)
- `[description]`: Plain text — everything after flags is the description
```

### Project Resolution Pattern

All commands that need project context use this identical block:

```markdown
### Project Resolution

If `--project` is provided → use it as the project slug.

If `--project` is omitted:

1. Scan `docs/dev/` for existing project folders
2. If folders exist → ask user to **select from the list** OR **type a new slug**
3. If no folders exist → ask user to **type a project slug**

Create `docs/dev/[project-slug]/` if it doesn't exist.
```

### Output File Structure

Documents are grouped by project and task under `docs/dev/`:

```
docs/dev/
└── [project-slug]/
    ├── product-discovery.md          # Project-level docs
    ├── solution-design.md
    ├── project-charter.md
    ├── project-plan.md
    └── [task-slug]/                  # Task-level docs
        ├── press-release.md
        ├── research.md
        └── implementation-plan.md
```

**Task slug format**:

- With `--ref`: `[ref-lowercase]-[description-kebab-case]` (e.g., `m1-tenant-creation`)
- Without `--ref`: `[description-kebab-case]` (e.g., `tenant-creation`)

### Handoff Pattern

Commands end with next-step instructions:

```markdown
## Next Steps

If you're satisfied with this [output], proceed to [next stage]:

/pipe-[next-command] --project [project-slug] --ref [ref] [description]

If you need changes:

- [How to request revisions]
```

### Pipeline Mode Pattern

Commands support automated pipeline execution:

```markdown
## Pipeline Mode

When called from `/pipe-pipeline`:

- Do NOT pause for user confirmation
- Return `OK` or `BLOCKING|[count]` at the end
- State management is handled by the pipeline orchestrator
```

## Conventions

- Filename: `pipe-[name].md` using kebab-case
- Title: Human-readable name (not the command name)
- `$ARGUMENTS` placeholder for user input
- Steps are numbered with clear names
- Critical Constraints section is always present
- Handoff section links to next pipeline stage

## Command Categories

| Category       | Commands                                                            | Purpose                 |
| -------------- | ------------------------------------------------------------------- | ----------------------- |
| Discovery      | `pipe-discover-product`                                             | Define product vision   |
| Planning       | `pipe-project-charter`, `pipe-project-plan`, `pipe-design-solution` | High-level planning     |
| Documentation  | `pipe-press-release`, `pipe-research`                               | Create specifications   |
| Review         | `pipe-review-*`                                                     | Quality gates           |
| Implementation | `pipe-plan-implementation`, `pipe-implement`                        | Build features          |
| Testing        | `pipe-e2e`                                                          | Generate E2E tests      |
| Utility        | `pipe-init`, `pipe-bugfix`                                          | Setup and maintenance   |
| Orchestration  | `pipe-pipeline`                                                     | Full automated workflow |

## Common Tasks

### Create New Command

1. Create `commands/pipe-[name].md`
2. Follow the Command File Structure pattern above
3. Include Parsing Arguments, Context, Your Role, Process, Critical Constraints
4. Add Handoff section if part of pipeline flow
5. Add Pipeline Mode section if command supports automation

### Test Command

```bash
# Install to test project
pipe-install ~/test-project
cd ~/test-project && claude
/pipe-[name] [test args]
```

## Related Files

| File                                   | Relationship                                   |
| -------------------------------------- | ---------------------------------------------- |
| `agents/*.md`                          | Commands spawn these via Task tool             |
| Target project's `AGENTS.md`           | Commands read this for project context         |
| `.pipeline/[project]-[ref]-state.json` | Pipeline state file (created by pipe-pipeline) |

## Gotchas

- Commands read target project's `AGENTS.md`, not this repo's
- `$ARGUMENTS` is a placeholder - it gets replaced with user input at runtime
- Review commands return `OK` or `BLOCKING|N` for pipeline integration
- Handoff sections must include the actual filename created (not a placeholder)
- Pipeline mode skips user confirmations - standalone mode pauses for approval
