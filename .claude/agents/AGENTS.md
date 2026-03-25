# Agents - AI Agent Guidelines

Generic agent definitions that are spawned as sub-tasks by pipeline commands.

> For project-wide guidelines, see [root AGENTS.md](../AGENTS.md)

## Overview

Agents are specialized workers spawned via the Task tool. Each agent has a narrow focus (locating files, analyzing code, running builds) and returns structured output. Agents read the target project's `AGENTS.md` at runtime - they contain no hardcoded project knowledge.

## Directory Structure

```
agents/
├── build-verifier.md         # Run builds and tests, report results
├── codebase-analyzer.md      # Explain HOW code works
├── codebase-locator.md       # Find WHERE things are
├── codebase-pattern-finder.md # Find existing patterns to follow
├── docs-analyzer.md          # Analyze documentation content
├── docs-locator.md           # Find documentation files
├── e2e-test-generator.md     # Generate Playwright E2E tests
└── web-researcher.md         # Research via web search
```

## Patterns

### Agent File Structure

Every agent follows this structure:

```markdown
# Agent: [name]

[One sentence role description]

## Tools Available
- [Tool 1] ([specific usage])
- [Tool 2] ([specific usage])

## Project Context
**IMPORTANT**: Read `AGENTS.md` for...

## Your Role
YOU ARE A [ROLE], NOT A [ANTI-ROLE].
[Clear constraints on what agent does NOT do]

## [Methodology Section]
[Step-by-step process]

## Output Format
[Template with placeholders]

## Critical Constraints
- [Constraint list]
```

### Role Constraint Pattern

Agents explicitly state what they do NOT do:

```markdown
## Your Role

YOU ARE A TECHNICAL DOCUMENTARIAN, NOT AN EVALUATOR.

You do NOT:
- Suggest improvements or changes
- Critique implementation
- Comment on code quality
```

### Runtime Context Pattern

Every agent reads target project's AGENTS.md first:

```markdown
## Project Context

**IMPORTANT**: Read `AGENTS.md` for the project structure and conventions.

Look for:
- `## Directory Structure` for project layout
- `## Tech Stack` for language/framework info
- `## Commands` for verification commands
```

## Conventions

- Filename: `kebab-case.md` matching the agent name
- Title format: `# Agent: [name]`
- Tools section lists available tools with usage notes
- Output format uses markdown code blocks with placeholders
- Critical Constraints section is always last

## Agent Categories

| Category | Agents | Purpose |
|----------|--------|---------|
| Discovery | `codebase-locator`, `docs-locator` | Find files and locations |
| Analysis | `codebase-analyzer`, `codebase-pattern-finder`, `docs-analyzer` | Understand code/docs |
| Execution | `build-verifier`, `e2e-test-generator` | Run commands, generate tests |
| External | `web-researcher` | Research via web |

## Common Tasks

### Create New Agent

1. Create `agents/[name].md`
2. Follow the Agent File Structure pattern above
3. Include Tools Available, Project Context, Your Role, Output Format, Critical Constraints
4. Add explicit "NOT" constraints for the role

### Test Agent Definition

```bash
# Install to test project and verify agent works
pipe-install ~/test-project
cd ~/test-project && claude
# Spawn agent via Task tool
```

## Related Files

| File | Relationship |
|------|--------------|
| `commands/*.md` | Commands spawn these agents via Task tool |
| Target project's `AGENTS.md` | Agents read this at runtime for context |

## Gotchas

- Agents have NO knowledge of specific projects - they read `AGENTS.md` at runtime
- Tool lists must match what the Task tool provides (Read, Grep, Glob, Bash, Write)
- "Your Role" section should explicitly state anti-patterns (what NOT to do)
- Output format templates must be copy-pasteable by the spawning command
