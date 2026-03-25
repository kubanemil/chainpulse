# Design Solution

Explore solution options for: $ARGUMENTS

## Purpose

This command facilitates **solution exploration and architecture design** - an exploratory phase where we understand the problem space, examine the current system, identify architectural options, and recommend an approach.

This is the **first step** before creating a project charter. It answers: "How might we solve this?"

**This produces:**

- Deep understanding of the problem
- Analysis of the current system
- Architectural options with trade-offs
- Recommended approach with rationale
- Key technical decisions and constraints

**This is NOT:**

- A project charter (use `/pipe-project-charter` after this)
- A detailed implementation plan (use `/pipe-plan-implementation` for that)
- Code or schema design (too early for that)

## Workflow Position

```
/pipe-design-solution  →  /pipe-project-charter  →  /pipe-press-release  →  /pipe-research  →  /pipe-plan-implementation  →  /pipe-implement
       ↑
  Solution Design
  (this document)
```

---

## I/O

**Input**: `[--project slug] [description]`
**Reads**: `docs/dev/[project-slug]/product-discovery.md` (if exists, for context)
**Output**: `docs/dev/[project-slug]/solution-design.md`
**Commit**:
```
[project-slug] solution-design: <short summary>

<description of the chosen approach and key trade-offs>
```

---

## Parsing Arguments

Arguments format: `[--project slug] [description]`

- `--project <slug>`: Project folder name under `docs/dev/`
- `[description]`: Plain text describing the problem to explore

### Project Resolution

If `--project` is provided → use it as the project slug.

If `--project` is omitted:

1. Scan `docs/dev/` for existing project folders
2. If folders exist → ask user to **select from the list** OR **type a new slug**
3. If no folders exist → ask user to **type a project slug**

Create `docs/dev/[project-slug]/` if it doesn't exist.

### Examples

| Command                                                            | Project Slug          | Output                                   |
| ------------------------------------------------------------------ | --------------------- | ---------------------------------------- |
| `/pipe-design-solution --project oauth-auth how to implement OIDC` | `oauth-auth`          | `docs/dev/oauth-auth/solution-design.md` |
| `/pipe-design-solution caching strategy` (projects exist)          | user selects or types | `docs/dev/[selected]/solution-design.md` |

---

## Your Role

You are a **technical architect** helping explore how to solve a problem. Your job is to:

1. Deeply understand the problem and its context
2. Explore the current system to find relevant patterns and constraints
3. Identify multiple architectural approaches
4. Analyse trade-offs honestly
5. Recommend an approach with clear rationale
6. Surface unknowns and risks early

---

## Process

### Phase 1: Understand the Problem

Start by gathering context. Ask the user:

1. **What problem are we solving?** - The pain point or opportunity
2. **Who experiences this problem?** - Users, stakeholders, systems
3. **What does success look like?** - Desired end state
4. **What constraints exist?** - Technical, business, timeline, resources
5. **What has been tried before?** - Previous attempts, learnings

If the user has already provided context (e.g., a brief, Loom video notes, or prior discussion), summarise your understanding and confirm before proceeding.

### Phase 2: Explore the Current System

**Use agents to research the codebase** - don't pollute the main context with implementation details.

Spawn parallel research agents to discover:

**Agent 1: Related Domain & Patterns**

```
Find existing patterns related to [problem domain]:
- Similar features or modules
- How related problems were solved
- Established conventions to follow
- Schemas, contexts, and services in this area
Return a summary of relevant patterns and approaches.
```

**Agent 2: Integration Points**

```
Identify integration points for [proposed capability]:
- What existing systems will this touch?
- What data does it need access to?
- What APIs or services does it depend on?
- What might depend on this new capability?
Return a map of integration points and dependencies.
```

**Agent 3: Technical Constraints**

```
Discover technical constraints relevant to [problem]:
- Performance requirements (scale, latency)
- Data constraints (volume, retention, privacy)
- Infrastructure constraints (hosting, queues, caching)
- Existing technical debt or limitations
Return a list of constraints that will shape the solution.
```

Synthesise the research findings into a **Current State Summary** for the user.

### Phase 3: Identify Architectural Options

Based on your understanding, identify **2-4 viable approaches** to solving the problem.

For each option, consider:

- How does it work at a high level?
- What are the key components?
- How does it integrate with existing systems?
- What are the trade-offs (pros/cons)?
- What's the complexity/effort level?
- What risks or unknowns does it introduce?

Present options neutrally first, then provide a recommendation.

### Phase 4: Recommend an Approach

After discussing options with the user:

1. Capture the chosen approach
2. Document key decisions made
3. Note any deferred decisions (to be resolved later)
4. Identify what needs further research

### Phase 5: Write the Solution Design Document

Write the document to: `docs/dev/[project-slug]/solution-design.md`

Create the `docs/dev/[project-slug]/` directory if it doesn't exist.

Use the **exact template** below.

### Phase 6: Handoff to Project Charter

Once the solution design is approved, the user can run `/pipe-project-charter` to turn the recommended approach into concrete scope, milestones, and work breakdown.

---

## Output Template

**IMPORTANT**: Follow this template. Keep it high-level - no code, no schemas.

```markdown
# [Project Name] - Solution Design

> **Status**: Draft | Ready for Review | Approved
> **Created**: [Date]
> **Last Updated**: [Date]

---

## 1. Problem Statement

### What problem are we solving?

[Clear description of the pain point or opportunity. Write for a non-technical reader.]

### Who is affected?

- [User type 1]: [How they experience the problem]
- [User type 2]: [How they experience the problem]

### What does success look like?

[Describe the desired end state. What can people do that they couldn't before?]

---

## 2. Current State

### Relevant Systems

| System/Module | Purpose        | Relevance to This Problem |
| ------------- | -------------- | ------------------------- |
| [System 1]    | [What it does] | [How it relates]          |
| [System 2]    | [What it does] | [How it relates]          |

### Existing Patterns We Can Leverage

| Pattern     | Where Used            | How It Helps        |
| ----------- | --------------------- | ------------------- |
| [Pattern 1] | [Example in codebase] | [Why it's relevant] |
| [Pattern 2] | [Example in codebase] | [Why it's relevant] |

### Constraints

| Constraint     | Type                        | Impact on Solution          |
| -------------- | --------------------------- | --------------------------- |
| [Constraint 1] | Technical/Business/Timeline | [How it shapes our options] |
| [Constraint 2] | Technical/Business/Timeline | [How it shapes our options] |

### Integration Points
```

[Simple ASCII diagram showing how the new capability fits into the existing system]

┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│ Existing │────▶│ NEW │────▶│ Existing │
│ System A │ │ Capability │ │ System B │
└─────────────┘ └─────────────┘ └─────────────┘

```

---

## 3. Solution Options

### Option A: [Name]

**Approach**: [1-2 sentence description of how this works]

**How it works**:
- [Key aspect 1]
- [Key aspect 2]
- [Key aspect 3]

**Pros**:
- [Advantage 1]
- [Advantage 2]

**Cons**:
- [Disadvantage 1]
- [Disadvantage 2]

**Complexity**: Low / Medium / High

**Unknowns**:
- [What we'd need to figure out]

---

### Option B: [Name]

**Approach**: [1-2 sentence description]

[Same structure as Option A]

---

### Option C: [Name] (if applicable)

[Same structure]

---

## 4. Recommendation

### Recommended Approach: [Option X]

**Rationale**:
[2-3 paragraphs explaining why this option is recommended. Reference the pros/cons from above. Acknowledge trade-offs.]

### Key Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| [What decision] | [What we chose] | [Why] |
| [What decision] | [What we chose] | [Why] |

### Deferred Decisions

| Decision | Options | When to Decide |
|----------|---------|----------------|
| [What needs to be decided later] | [Known options] | [During which phase] |

---

## 5. High-Level Architecture

[ASCII diagram of the recommended approach - keep it simple, show major components and data flow]

```

┌─────────────────────────────────────────────────────────────┐
│ [System Name] │
│ │
│ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ │
│ │ Component A │────▶│ Component B │────▶│ Component C │ │
│ │ │ │ (NEW) │ │ │ │
│ └──────────────┘ └──────────────┘ └──────────────┘ │
│ │ │
│ ▼ │
│ ┌──────────────┐ │
│ │ Component D │ │
│ │ (NEW) │ │
│ └──────────────┘ │
└─────────────────────────────────────────────────────────────┘

```

### Component Responsibilities

| Component | Responsibility | New/Existing |
|-----------|---------------|--------------|
| [Component 1] | [What it does] | Existing |
| [Component 2] | [What it does] | New |

---

## 6. Risks & Unknowns

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| [What could go wrong] | High/Medium/Low | High/Medium/Low | [How we'll address it] |

### Open Questions

- [ ] [Question that needs research]
- [ ] [Question that needs exploration]
- [ ] [Question that needs stakeholder input]

---

## 7. Next Steps

1. [ ] Review and approve this solution design
2. [ ] Resolve open questions: [list critical ones]
3. [ ] Create project charter: `/pipe-project-charter --project [project-slug]`

---

## Appendix: Research Notes (optional)

[Any additional context from codebase exploration that might be useful later]
```

---

## Critical Rules

### DO Include:

- **Problem context** - Why are we doing this?
- **Current system analysis** - What exists today?
- **Multiple options** - At least 2 approaches with trade-offs
- **Clear recommendation** - With rationale
- **High-level architecture** - Components and data flow (ASCII diagrams)
- **Risks and unknowns** - What could go wrong?

### DO NOT Include:

- ❌ Database schemas or column definitions
- ❌ Code examples or implementation details
- ❌ File paths or module names
- ❌ API contracts (GraphQL/REST definitions)
- ❌ Detailed algorithms
- ❌ Specific libraries or dependencies (unless critical to the decision)

### Level of Detail:

- "We need a table to track runs" ✅
- "Table with columns: id, status, created_at..." ❌
- "Background job processing" ✅
- "Oban worker in lib/backend/workers/..." ❌
- "Real-time updates to the UI" ✅
- "Phoenix PubSub with topic autoscheduling:#{id}" ❌

---

## Collaboration Tips

### When exploring options:

- Present trade-offs neutrally before recommending
- Ask the user which constraints matter most to them
- Surface hidden assumptions ("I'm assuming X, is that correct?")

### When recommending:

- Acknowledge what you're giving up with this choice
- Explain why the trade-offs are acceptable
- Be clear about what's still unknown

### When stuck:

- Ask clarifying questions
- Propose a "spike" or research task
- Mark decisions as "deferred" rather than guessing

---

Start by asking the user about the problem they want to explore, or if they've already described it, summarise your understanding and confirm.
