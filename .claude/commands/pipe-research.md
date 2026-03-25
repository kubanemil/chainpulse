# Research Codebase

Research the codebase to understand architecture, patterns, and implementation details related to: $ARGUMENTS

## I/O

**Input**: `[--project slug] [--ref id] [topic-description]`
**Reads**: `docs/dev/[project-slug]/[task-slug]/press-release.md` (if exists, for context)
**Output**: `docs/dev/[project-slug]/[task-slug]/research.md`

---

## Parsing Arguments

Arguments format: `[--project slug] [--ref id] [topic-description]`

- `--project <slug>`: Project folder name under `docs/dev/`
- `--ref <id>`: Optional. Milestone (`m1`, `M0.3`), ticket (`PROJ-123`), or issue (`#42`)
- `[topic-description]`: Plain text — everything after flags is the description

### Project Resolution

If `--project` is provided → use it as the project slug.

If `--project` is omitted:

1. Scan `docs/dev/` for existing project folders
2. If folders exist → ask user to **select from the list** OR **type a new slug**
3. If no folders exist → ask user to **type a project slug**

Create `docs/dev/[project-slug]/` if it doesn't exist.

### Task Slug

The task slug identifies the task folder within the project:

- With reference: `[ref-lowercase]-[topic-kebab-case]` (e.g., `m1-tenant-creation`)
- Without reference: `[topic-kebab-case]` (e.g., `event-sourcing-patterns`)

Rules: lowercase, kebab-case, max 4 words for description.

### Examples

| Command                                                       | Project      | Task Slug            | Output                                              |
| ------------------------------------------------------------- | ------------ | -------------------- | --------------------------------------------------- |
| `/pipe-research --project workforce --ref m1 tenant creation` | `workforce`  | `m1-tenant-creation` | `docs/dev/workforce/m1-tenant-creation/research.md` |
| `/pipe-research --project oauth auth patterns`                | `oauth`      | `auth-patterns`      | `docs/dev/oauth/auth-patterns/research.md`          |
| `/pipe-research caching strategy` (projects exist)            | user selects | `caching-strategy`   | `docs/dev/[selected]/caching-strategy/research.md`  |

## Context

First, read `AGENTS.md` to understand the project structure and conventions.

## Your Role

You are a **documentarian**, not an evaluator. Your job is to:

- Document the codebase AS-IS
- Provide factual observations with file paths and line numbers
- NOT suggest improvements, identify problems, or critique architecture

## Research Process

### Step 1: Initial File Reading

- Read `AGENTS.md` for project context
- Read ALL files mentioned by the user completely
- Do NOT spawn sub-tasks before reading these files yourself
- Understand the context before decomposing the research

### Step 2: Decompose Research Question

Break down the research into composable areas:

- Core implementation files
- Related utilities/helpers
- Configuration and setup
- Tests (if relevant)
- Dependencies and integrations

### Step 3: Parallel Research with Specialized Agents

Spawn parallel sub-agents using the Task tool for comprehensive coverage:

**File Discovery** - Use agent definition from `.claude/agents/codebase-locator.md`:

```
Find all files related to [topic]. Search for:
- Implementation files
- Test files
- Configuration
- Related modules
```

**Code Analysis** - Use agent definition from `.claude/agents/codebase-analyzer.md`:

```
Analyze how [component] works. Trace:
- Entry points and public APIs
- Data flow through the system
- Key functions and their purposes
```

**Pattern Discovery** - Use agent definition from `.claude/agents/codebase-pattern-finder.md`:

```
Find existing patterns related to [topic]. Look for:
- Similar implementations we can reference
- Conventions used in this area
- Test patterns for this type of code
```

**Documentation Search** - Use agent definition from `.claude/agents/docs-locator.md`:

```
Find existing documentation about [topic] in docs/ directory
```

### Step 4: Compile Findings

Wait for ALL agents to complete, then synthesize into a structured document.

### Step 5: Generate Research Document

Create a markdown file at: `docs/dev/[project-slug]/[task-slug]/research.md`

**Task slug format**:

- With reference: `[reference]-[topic-slug]` (e.g., `m0.3-tenant-creation`, `proj-123-auth-system`)
- Without reference: `[topic-slug]` (e.g., `event-sourcing-patterns`)

Rules:

- Convert reference to lowercase
- Use kebab-case for the topic slug
- The file is always named `research.md` — the task slug is the folder name
- Create the `docs/dev/[project-slug]/[task-slug]/` directory if it doesn't exist

Use this structure:

```markdown
---
date: [ISO date]
researcher: [model name from session]
branch: [current git branch]
repository: [from AGENTS.md]
tags: [relevant, tags]
status: complete
---

# Research: [Topic]

## Research Question

[The original question or area being researched]

## Summary

[2-3 paragraph executive summary of findings]

## Detailed Findings

### [Area 1]

[Findings with specific file paths and line numbers]

### [Area 2]

[Findings with specific file paths and line numbers]

## Code References

- `path/to/file:123` - [Description]
- `path/to/file:456` - [Description]

## Architecture Notes

[How the components fit together]

## Open Questions

[Any unresolved questions that need follow-up]
```

### Step 6: Sync and Present

- Write the research document to the file
- Present a summary to the user
- Ask if they have follow-up questions

## Critical Constraints

- **Document only**: No suggestions, improvements, or critiques
- **Be specific**: Include file paths and line numbers
- **Stay factual**: Report what IS, not what SHOULD BE
- **Be thorough**: Cover all relevant aspects of the question
- **Use agents**: Leverage specialized agents for parallel research

## Follow-up Questions

If the user asks follow-up questions, append findings to the SAME research document under a new section:

```markdown
## Follow-up: [Question]

[Additional findings]
```

## Handoff to Planning

**IMPORTANT**: At the end of EVERY response where you present research findings (initial or follow-up), you MUST include a "Next Steps" section:

```markdown
---

## Next Steps

If you're satisfied with this research, proceed to planning:

/pipe-plan-implementation --project [project-slug] --ref [ref] --research docs/dev/[project-slug]/[task-slug]/research.md [feature/task name]

If you need more research:

- Ask follow-up questions and I'll append to this document
- Or request a new research topic with /pipe-research --project [project-slug] --ref [ref] [new-topic]
```

Replace `[project-slug]` and `[task-slug]` with the actual values. Include the same reference (e.g., `m0.3`, `PROJ-123`) if one was used for this research, or omit if none.

Start by reading `AGENTS.md` and any files the user mentioned, then proceed with the research.
