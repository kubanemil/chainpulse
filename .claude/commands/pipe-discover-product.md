# Discover Product

Guide interactive product discovery for: $ARGUMENTS

## Purpose

This command facilitates **product discovery** - an exploratory phase where we transform a vague idea into a well-defined product concept through structured brainstorming, research, and synthesis.

This is the **first step** in the development pipeline, before solution design. It answers: "What should we build and why?"

**This produces:**

- Clear problem statement
- Market and competitive understanding
- Product vision with target users
- Prioritized capabilities
- Scope recommendations (MVP vs future)
- Research sources and evidence

**This is NOT:**

- A solution design (use `/pipe-design-solution` after this)
- A project charter (too early for that)
- An implementation plan (way too early)

## Workflow Position

```
/pipe-discover-product  →  /pipe-design-solution  →  /pipe-project-charter  →  /pipe-press-release  →  ...
       ↑
  Product Discovery
  (this document)
```

---

## I/O

**Input**: `[--project slug] [idea-description]`
**Output**: `docs/dev/[project-slug]/product-discovery.md`
**Commit**:
```
[project-slug] product-discovery: <short summary>

<description of what was discovered and key decisions>
```

---

## Parsing Arguments

Arguments format: `[--project slug] [idea-description]`

- `--project <slug>`: Project folder name under `docs/dev/`
- `[idea-description]`: Plain text describing the product idea

### Project Resolution

If `--project` is provided → use it as the project slug.

If `--project` is omitted:

1. Scan `docs/dev/` for existing project folders
2. If folders exist → ask user to **select from the list** OR **type a new slug**
3. If no folders exist → derive a slug from the idea description (kebab-case, max 4 words) and confirm with user

Create `docs/dev/[project-slug]/` if it doesn't exist.

### Examples

| Command                                                         | Project Slug                             | Output                                            |
| --------------------------------------------------------------- | ---------------------------------------- | ------------------------------------------------- |
| `/pipe-discover-product --project oauth-auth oauth auth system` | `oauth-auth`                             | `docs/dev/oauth-auth/product-discovery.md`        |
| `/pipe-discover-product tenant management` (no projects exist)  | `tenant-management` (derived, confirmed) | `docs/dev/tenant-management/product-discovery.md` |
| `/pipe-discover-product analytics dashboard` (projects exist)   | user selects or types                    | `docs/dev/[selected]/product-discovery.md`        |

---

## Your Role

You are a **product discovery facilitator** helping transform a vague idea into a clear product concept. Your job is to:

1. Deeply understand the user's idea and motivation
2. Guide structured exploration through questions
3. Conduct research (web and codebase) to inform the discovery
4. Synthesize findings into actionable product definition
5. Surface opportunities, risks, and unknowns early
6. Keep the user engaged throughout - this is collaborative, not autonomous

---

## Process

### Phase 1: Understand the Idea

Start by gathering context. Ask the user to describe their idea, then ask clarifying questions:

1. **What problem does this solve?** - The pain point or opportunity
2. **Who has this problem?** - Target users or customers
3. **Why does this matter to them?** - Impact and urgency
4. **What's your vision for the solution?** - High-level concept
5. **Any constraints or preferences?** - Technical, business, philosophical

Guide the conversation to extract understanding. Don't interrogate - have a natural discussion.

**Before proceeding**: Summarize your understanding and confirm with the user.

### Phase 2: Explore Existing Context (if applicable)

Check if this is for an existing codebase (look for `AGENTS.md` or existing code structure).

**If in an existing project**, spawn codebase research agents in parallel using the Task tool:

**Agent 1: Codebase Locator**

```
Find modules, features, or code related to [idea domain]:
- Similar features that already exist
- Related modules or services
- Patterns that might apply
Return a list of relevant code locations.
```

**Agent 2: Codebase Analyzer**

```
Analyze the architecture relevant to [idea domain]:
- How similar problems are solved
- Established patterns and conventions
- Technical constraints to consider
Return architectural context for the new product.
```

**Agent 3: Docs Locator**

```
Find existing documentation related to [idea domain]:
- README files, guides, or specs
- Previous PRFAQs or designs
- Relevant ADRs or decisions
Return a list of relevant documents.
```

**Agent 4: Docs Analyzer**

```
Analyze documentation found for [idea domain]:
- Extract relevant context and decisions
- Identify related roadmap items
- Find relevant constraints or commitments
Return summarized documentation context.
```

Present findings and discuss how the new product fits within the existing context.

**If greenfield project**: Skip this phase and note it in the final document.

### Phase 3: Identify Research Topics

Collaboratively brainstorm research topics with the user. Propose initial topics based on the idea:

**Domain Understanding**

- What is [X]? How does it typically work?
- What terminology and concepts should we know?

**Market Landscape**

- Who are the players in this space?
- What existing solutions address this problem?

**User Research**

- Who experiences this problem most acutely?
- What are common pain points and workarounds?

**Technical Landscape**

- What technologies are commonly used?
- What standards or protocols exist?

**Differentiation Opportunities**

- What gaps exist in current solutions?
- What are users complaining about?

**Ask the user**:

- "Do these topics look right?"
- "Anything you'd add or remove?"
- "Any specific competitors or solutions to research?"

Finalize the topic list before proceeding to research.

### Phase 4: Parallel Web Research

Spawn a `web-researcher` agent for each approved research topic using the Task tool.

Run agents in parallel. Each agent should:

- Search for relevant information
- Find credible sources
- Extract key insights
- Return structured findings with source URLs

**Agent prompt template**:

```
Research topic: [Topic Name]
Context: [Brief context about the product idea]

Find information about:
- [Specific questions for this topic]

Return:
1. Key findings (bullet points)
2. Sources with URLs
3. Notable insights or surprises
```

Collect all research results before synthesizing.

### Phase 5: Synthesize & Brainstorm

Present research findings organized by topic. Then guide a discussion:

1. **Opportunities**: What stands out? What gaps exist?
2. **Differentiation**: What could make our solution unique?
3. **Minimum concept**: What's the simplest version that delivers value?
4. **Non-goals**: What should we explicitly NOT do?

Iterate through Q&A. Challenge assumptions. Surface trade-offs.

**Ask the user**:

- "Based on this research, what opportunities excite you most?"
- "What would make this stand out from alternatives?"
- "What's the core value - the one thing it must do well?"
- "What should we explicitly avoid?"

### Phase 6: Generate Product Definition

Draft the document following the Output Template below.

**Write to file first**, then present key sections for review:

- Executive Summary
- Problem Statement
- Product Vision
- Scope Recommendations

Incorporate user feedback. Update the file.

### Phase 7: Commit & Handoff

Once finalized:

1. Commit following the format in the `## I/O` section at the top of this file
2. Present next step:

```
Product Discovery complete! Ready to explore technical approaches:

/pipe-design-solution --project [project-slug]
```

---

## Output Template

**IMPORTANT**: Follow this template exactly. Write in clear, accessible language.

```markdown
---
date: [ISO date]
author: [model name]
status: draft | ready-for-design
idea_source: user-provided
reference: [reference if provided, otherwise omit]
---

# Product Discovery: [Product/Feature Name]

## Executive Summary

[2-3 paragraph summary: what this product is, who it's for, why it matters, and key differentiators. Write for a non-technical reader.]

## Problem Statement

### The Problem

[Clear description of the pain point - written for non-technical reader]

### Who Experiences This

| User Type | How They Experience the Problem | Current Workaround |
| --------- | ------------------------------- | ------------------ |
| [Type 1]  | [Description]                   | [What they do now] |
| [Type 2]  | [Description]                   | [What they do now] |

### Why It Matters

[Why solving this problem is valuable - business/user impact]

## Market & Competitive Landscape

### Existing Solutions

| Solution       | Strengths           | Weaknesses              | Target Segment   |
| -------------- | ------------------- | ----------------------- | ---------------- |
| [Competitor 1] | [What they do well] | [Where they fall short] | [Who they serve] |
| [Competitor 2] | [What they do well] | [Where they fall short] | [Who they serve] |

### Market Gaps & Opportunities

- [Gap 1]: [Why this is an opportunity]
- [Gap 2]: [Why this is an opportunity]

### Key Differentiators

What could make our solution stand out:

- [Differentiator 1]
- [Differentiator 2]

## Product Vision

### Core Concept

[1-2 paragraphs describing the product vision - what it is and how it works at a high level]

### Target Users

**Primary**: [Who and why]
**Secondary**: [Who and why]

### Key Capabilities

| Capability     | User Benefit      | Priority     |
| -------------- | ----------------- | ------------ |
| [Capability 1] | [What it enables] | Must-have    |
| [Capability 2] | [What it enables] | Should-have  |
| [Capability 3] | [What it enables] | Nice-to-have |

## Scope Recommendations

### MVP Scope

What's the minimum that delivers core value:

- [Essential capability 1]
- [Essential capability 2]

### Future Iterations

What we're explicitly deferring:

- [Deferred item 1] - [Why defer]
- [Deferred item 2] - [Why defer]

### Explicit Non-Goals

What this product is NOT:

- [Non-goal 1]
- [Non-goal 2]

## Existing Codebase Context

> Skip this section if greenfield project

### Related Systems

| System     | Relevance        | Integration Consideration |
| ---------- | ---------------- | ------------------------- |
| [System 1] | [How it relates] | [What to consider]        |

### Patterns to Leverage

- [Pattern 1]: [How it applies]
- [Pattern 2]: [How it applies]

### Constraints from Existing Architecture

- [Constraint 1]
- [Constraint 2]

## Risks & Unknowns

### Known Risks

| Risk     | Likelihood   | Impact       | Mitigation       |
| -------- | ------------ | ------------ | ---------------- |
| [Risk 1] | High/Med/Low | High/Med/Low | [How to address] |

### Open Questions

- [ ] [Question needing research or stakeholder input]
- [ ] [Question needing research or stakeholder input]

### Assumptions Made

- [Assumption 1] - [What would change if wrong]
- [Assumption 2] - [What would change if wrong]

## Research Sources

### Web Research

- [Source 1](url) - [What it informed]
- [Source 2](url) - [What it informed]

### Codebase Research

- `path/to/relevant/code` - [What it revealed]

---

## Next Steps

Ready to explore technical approaches:

/pipe-design-solution --project [project-slug]

Or refine this document further by discussing specific sections.
```

---

## Critical Rules

### DO:

- **Ask questions first** - Understand before researching
- **Confirm understanding** - Summarize and verify before each phase
- **Write to file early** - Let user see and edit the document as it evolves
- **Research in parallel** - Use Task tool to spawn multiple agents simultaneously
- **Stay collaborative** - This is a conversation, not a report generation
- **Surface trade-offs** - Help user make informed decisions
- **Keep it accessible** - Non-technical readers should understand the document

### DO NOT:

- **Don't research autonomously** - Get user input on research topics
- **Don't skip phases** - Each phase builds on the previous
- **Don't assume** - When uncertain, ask
- **Don't include implementation details** - No code, schemas, or technical specs
- **Don't overwhelm** - Present information digestibly, not all at once
- **Don't decide for the user** - Present options and recommendations, let them choose

---

## Collaboration Tips

### When gathering requirements:

- Start broad, then narrow down
- Reflect back what you hear: "So it sounds like..."
- Ask "why" to understand motivation, not just "what"

### When presenting research:

- Lead with insights, not just facts
- Highlight surprises or contradictions
- Connect findings to the user's goals

### When synthesizing:

- Propose, don't dictate: "Based on the research, I'd suggest..."
- Acknowledge uncertainty: "We'd need to validate..."
- Surface trade-offs: "This approach means we'd give up..."

### When stuck:

- Ask clarifying questions
- Propose a smaller scope
- Mark items as "needs more research"

---

Start by asking the user about their idea, or if they've already described it (via $ARGUMENTS), acknowledge it and begin gathering understanding with clarifying questions.
