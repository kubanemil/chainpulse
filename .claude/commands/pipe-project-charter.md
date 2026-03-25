# Create Project Charter

Create a project charter for: $ARGUMENTS

## Purpose

This command creates a **Project Charter** - a high-level document that defines what a project will deliver, its scope, key decisions, and work breakdown. It is the strategic foundation that stakeholders (technical and non-technical) can review to understand what we're building and why.

**This is NOT:**

- A Solution Design (use `/pipe-design-solution` for architecture exploration)
- A Technical Design Document (no schemas, no code, no file paths)
- A detailed PRD (use `/pipe-press-release` for individual features)
- An implementation plan (use `/pipe-plan-implementation` for that)

## Workflow Position

```
/pipe-design-solution  →  /pipe-project-charter  →  /pipe-press-release (per feature)  →  /pipe-research  →  /pipe-plan-implementation  →  /pipe-implement
                           ↑
                     Project Charter
                     (this document)
```

**Prerequisites**: Ideally, run `/pipe-design-solution` first to explore architecture options. The project charter then takes the chosen approach and turns it into concrete scope and work breakdown.

---

## I/O

**Input**: `[--project slug] [description]`
**Reads**: `docs/dev/[project-slug]/solution-design.md` (if exists, for context)
**Output**: `docs/dev/[project-slug]/project-charter.md`
**Commit**:
```
[project-slug] project-charter: <short summary>

<description of scope, milestones, and key decisions>
```

---

## Parsing Arguments

Arguments format: `[--project slug] [description]`

- `--project <slug>`: Project folder name under `docs/dev/`
- `[description]`: Plain text describing the project to charter

### Project Resolution

If `--project` is provided → use it as the project slug.

If `--project` is omitted:

1. Scan `docs/dev/` for existing project folders
2. If folders exist → ask user to **select from the list** OR **type a new slug**
3. If no folders exist → ask user to **type a project slug**

Create `docs/dev/[project-slug]/` if it doesn't exist.

### Examples

| Command                                                      | Project Slug          | Output                                   |
| ------------------------------------------------------------ | --------------------- | ---------------------------------------- |
| `/pipe-project-charter --project oauth-auth`                 | `oauth-auth`          | `docs/dev/oauth-auth/project-charter.md` |
| `/pipe-project-charter billing integration` (projects exist) | user selects or types | `docs/dev/[selected]/project-charter.md` |

---

## Your Role

You are a **product strategist** helping define a project's scope and deliverables. Your job is to:

1. Understand the problem and desired end state
2. Ask clarifying questions - do NOT assume
3. Define clear boundaries (in scope / out of scope)
4. Break work into milestones, epics, and user stories
5. Identify decisions and unknowns
6. Keep everything at the appropriate abstraction level

---

## Process

### Phase 1: Understand the Project

Ask the user to describe:

1. **The problem**: What pain point or opportunity are we addressing?
2. **The end state**: What will exist when this project is complete?
3. **The stakeholders**: Who cares about this? Who will use it?
4. **Known constraints**: Any non-negotiable technical or business requirements?

If the user has already provided context, summarise your understanding and confirm before proceeding.

### Phase 2: Draft the Project Plan

Write the document to: `docs/dev/[project-slug]/project-charter.md`

Create the `docs/dev/[project-slug]/` directory if it doesn't exist.

Use the **exact template** below. Fill in each section based on your conversation.

### Phase 3: Review and Refine

Present the draft section by section. Ask:

- "Does the End State Vision capture what success looks like?"
- "Is the scope correct? Anything to add to Out of Scope?"
- "Are the milestones in the right order?"
- "Any decisions I've marked as Open that are actually Decided?"

Update the document after each round of feedback.

### Phase 4: Finalise

When the user approves, commit following the format in the `## I/O` section at the top of this file

---

## Output Template

**IMPORTANT**: Follow this template exactly. Do not add sections. Do not include implementation details.

```markdown
# [Project Name] - Project Plan

> **Status**: Draft | Ready for Review | Approved
> **Created**: [Date]
> **Last Updated**: [Date]

---

## 1. End State Vision

**By completing this project, we will have:**

- [Concrete deliverable 1 - what will exist]
- [Concrete deliverable 2 - what users can do]
- [Concrete deliverable 3 - what data/capability we gain]

**A stakeholder can verify success by:**

[Observable outcome - how someone can confirm the project succeeded]

---

## 2. Problem Statement

[2-3 paragraphs max. Describe the current pain point, why it matters, and who is affected. Write for a non-technical reader.]

---

## 3. Scope

### In Scope

- [Capability/feature 1]
- [Capability/feature 2]
- [Capability/feature 3]

### Out of Scope (Non-Goals)

- [Explicitly excluded item 1] - _[why excluded]_
- [Explicitly excluded item 2] - _[why excluded]_
- [Explicitly excluded item 3] - _[why excluded]_

---

## 4. Key Decisions

### Decided

| ID  | Decision         | Choice          | Rationale |
| --- | ---------------- | --------------- | --------- |
| D1  | [What decision?] | [What we chose] | [Why]     |
| D2  | [What decision?] | [What we chose] | [Why]     |

### Open (Needs Research)

| ID  | Question                    | Options         | Decide By   |
| --- | --------------------------- | --------------- | ----------- |
| O1  | [What needs to be decided?] | [Known options] | [Milestone] |
| O2  | [What needs to be decided?] | [Known options] | [Milestone] |

---

## 5. Milestones

| ID  | Milestone | Deliverable                               | Depends On |
| --- | --------- | ----------------------------------------- | ---------- |
| M1  | [Name]    | [What's delivered - user-visible outcome] | -          |
| M2  | [Name]    | [What's delivered - user-visible outcome] | M1         |
| M3  | [Name]    | [What's delivered - user-visible outcome] | M1, M2     |

---

## 6. Work Breakdown

### M1: [Milestone Name]

**Deliverable**: [What users/stakeholders get at the end of this milestone]

#### E1.1: [Epic Name]

| ID     | User Story                                    | Acceptance Criteria | Size  | Depends On |
| ------ | --------------------------------------------- | ------------------- | ----- | ---------- |
| E1.1.1 | As a [role], I can [action] so that [benefit] | [How to verify]     | S/M/L | -          |
| E1.1.2 | As a [role], I can [action] so that [benefit] | [How to verify]     | S/M/L | E1.1.1     |

#### E1.2: [Epic Name]

| ID     | User Story                                    | Acceptance Criteria | Size  | Depends On |
| ------ | --------------------------------------------- | ------------------- | ----- | ---------- |
| E1.2.1 | As a [role], I can [action] so that [benefit] | [How to verify]     | S/M/L | E1.1.1     |

### M2: [Milestone Name]

**Deliverable**: [What users/stakeholders get at the end of this milestone]

[Continue pattern...]

---

## 7. Technical Approach (High-Level)

[1-2 paragraphs describing the general approach. NO code, NO schemas, NO file paths.]

**Key architectural choices:**

- [Choice 1]: [Brief rationale]
- [Choice 2]: [Brief rationale]

**Patterns we'll follow:**

- [Existing pattern in codebase to reuse]
- [Existing pattern in codebase to reuse]

---

## 8. Risks & Unknowns

| Risk                  | Impact          | Likelihood      | Mitigation             |
| --------------------- | --------------- | --------------- | ---------------------- |
| [What could go wrong] | High/Medium/Low | High/Medium/Low | [How we'll address it] |

### Open Questions

- [ ] [Question that needs research before implementation]
- [ ] [Question that needs research before implementation]

---

## 9. Success Criteria

### [Milestone 1] is complete when:

- [ ] [Observable outcome 1]
- [ ] [Observable outcome 2]

### [Milestone 2] is complete when:

- [ ] [Observable outcome 1]
- [ ] [Observable outcome 2]

---

## 10. Next Steps

1. Review and approve this project plan
2. For each milestone, create Press Release docs: `/pipe-press-release --project [project-slug] --ref [ref] [feature]`
3. Research implementation details: `/pipe-research --project [project-slug] --ref [ref] [area]`
4. Create implementation plans: `/pipe-plan-implementation --project [project-slug] --ref [ref] [feature]`
5. Execute: `/pipe-implement docs/dev/[project-slug]/[task-slug]/implementation-plan.md`

---

## Appendix: Glossary (if needed)

| Term          | Definition                   |
| ------------- | ---------------------------- |
| [Domain term] | [Plain language explanation] |
```

---

## Critical Rules

### DO Include:

- **End state vision** - What will exist when done
- **User stories** - "As a [role], I can [action]"
- **High-level technical choices** - "We'll use Oban for background jobs"
- **Risks and unknowns** - What could block us
- **Dependencies between work items** - What must happen first
- **Success criteria** - How to verify completion

### DO NOT Include:

- ❌ Database schemas or migrations
- ❌ Code examples or snippets
- ❌ File paths or module names
- ❌ API designs (GraphQL/REST)
- ❌ Query patterns
- ❌ Detailed algorithms
- ❌ UI mockups or wireframes

When tempted to add technical details, instead write:

- "We need to persist run data" (not the schema)
- "Background processing required" (not the Oban worker code)
- "Real-time updates to UI" (not the PubSub implementation)

### Defer to Later Phases:

- Schema design → `/pipe-research` then `/pipe-plan-implementation`
- API design → `/pipe-research` then `/pipe-plan-implementation`
- Implementation details → `/pipe-plan-implementation` then `/pipe-implement`
- File structure → `/pipe-plan-implementation`

---

## Size Guidelines

| Size | Meaning   | Typical Scope                            |
| ---- | --------- | ---------------------------------------- |
| S    | Small     | Single function, simple change, < 1 day  |
| M    | Medium    | Multiple functions, 1-3 days             |
| L    | Large     | Cross-cutting, needs breakdown, 3-5 days |
| XL   | Too Large | Must be split into smaller stories       |

If a story is XL, break it into multiple M/L stories.

---

## Dependency Tracking

Dependencies enable parallel work planning. Mark stories as:

- **No dependency (-)**: Can start immediately, parallelisable
- **Depends on [ID]**: Must wait for that story to complete

Stories with no dependencies can be worked on in parallel by multiple agents.

---

## Example: Good vs Bad

### Bad (Too Technical):

```
E1.1.1: Create `autoscheduling_runs` table with columns: id (uuid),
organisation_id (uuid FK), status (enum: pending, running, completed)...
```

### Good (Right Level):

```
E1.1.1: As a developer, I can persist autoscheduling run data so that
we have a history of all runs. Acceptance: Run records are created and
queryable.
```

### Bad (Implementation Detail):

```
E1.3.1: Create AutoschedulingRunWorker Oban worker in
lib/backend/workers/ that calls AutoschedulingService.auto_assign_open_shifts/4
```

### Good (Right Level):

```
E1.3.1: As a system, autoscheduling runs execute in the background so that
users can navigate away. Acceptance: User triggers run, can leave page,
run completes.
```

---

Start by asking the user about the project they want to plan, or if they've already described it, summarise your understanding and confirm.
