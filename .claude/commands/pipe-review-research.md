# Review Research Document

Review a research document for completeness, accuracy, and alignment with Press Release requirements: $ARGUMENTS

## Purpose

This command validates that a research document:

1. Addresses the questions and requirements from the related Press Release
2. Has accurate code references with file paths and line numbers
3. Documents the codebase AS-IS (no suggestions or critiques)
4. Covers all relevant areas of the codebase
5. Identifies open questions that need resolution before planning

## I/O

**Input**: `[--project slug] [research-path] [--press-release path]`
**Reads**: research + sibling `press-release.md` in same task directory
**Output**: Review report (presented to user, not written to file)

---

## Parsing Arguments

Arguments format: `[--project slug] [research-path] [--press-release path]`

- `--project <slug>`: Project folder — used to find research docs when no direct path given
- `[research-path]`: Direct path to the research document to review
- `--press-release <path>`: Explicit path to related press release (overrides sibling lookup)

### Document Resolution

**If a direct `research-path` is provided** → use it. Look for sibling `press-release.md` in the same directory.

**If no path but `--project` is provided** → list `research.md` files in that project folder, ask user to choose.

**If neither is provided**:

1. Scan `docs/dev/` for existing project folders
2. If folders exist → ask user to **select a project**, then **select a research doc**
3. If no folders exist → ask user for the research path

### Examples

| Command                                                    | Behavior                                        |
| ---------------------------------------------------------- | ----------------------------------------------- |
| `/pipe-review-research docs/dev/oauth/m1-oidc/research.md` | Review that doc, auto-find sibling PR           |
| `/pipe-review-research --project oauth`                    | List research docs in `oauth`, ask user to pick |
| `/pipe-review-research`                                    | Ask user to select project, then research doc   |

## Your Role

You are a **quality gate reviewer**. Your job is to:

- Verify the research document is complete and well-referenced
- Ensure all claims have supporting evidence (file:line references)
- Check that research stays factual (no opinions or suggestions)
- Identify gaps in coverage that could block planning
- Present findings as a blocking checklist

## Review Process

### Step 1: Read Documents

1. Read `AGENTS.md` for project context
2. Read the research document completely
3. If `--press-release` provided, read it for scope cross-reference

### Step 2: Execute Review Checklist

Work through each category below. For each criterion:

- **✅ PASS**: Criterion fully met
- **⚠️ WARN**: Minor issue, but acceptable
- **❌ FAIL**: Critical issue that must be fixed

---

## Review Checklist

### 1. Document Structure

| Criterion                                                                 | Check |
| ------------------------------------------------------------------------- | ----- |
| Has YAML frontmatter (date, researcher, branch, repository, tags, status) |       |
| Has clear Research Question section                                       |       |
| Has Summary section (2-3 paragraphs)                                      |       |
| Has Detailed Findings with subsections                                    |       |
| Has Code References section                                               |       |
| Has Architecture Notes section                                            |       |
| Has Open Questions section                                                |       |

### 2. Reference Quality

| Criterion                                             | Check |
| ----------------------------------------------------- | ----- |
| All file paths use `path/to/file.ex:line` format      |       |
| Line numbers are accurate (spot-check 3-5 references) |       |
| References actually exist in codebase                 |       |
| Key claims have supporting references                 |       |
| No vague references like "in the codebase"            |       |

### 3. Factual Objectivity

| Criterion                             | Check |
| ------------------------------------- | ----- |
| Describes what IS, not what SHOULD BE |       |
| No suggestions for improvement        |       |
| No critique of existing architecture  |       |
| No opinions on code quality           |       |
| Language is neutral and observational |       |

**Anti-patterns to flag**:

- Suggestions: "should be", "could be better", "recommend"
- Critiques: "this is wrong", "bad pattern", "technical debt"
- Proposals: "we need to", "the solution is"
- Judgments: "better than", "worse than", "problematic"

**Acceptable content**:

- Factual: "the code does X", "this pattern is used for Y"
- Observations: "there are N instances of this pattern"
- Questions: "unclear why X approach was chosen"

### 4. Coverage Completeness

| Criterion                              | Check |
| -------------------------------------- | ----- |
| Core implementation files identified   |       |
| Related utilities/helpers covered      |       |
| Configuration and setup documented     |       |
| Test patterns researched (if relevant) |       |
| Dependencies and integrations mapped   |       |
| Entry points clearly identified        |       |
| Data flow traced end-to-end            |       |

**Expected coverage areas** (adjust based on feature):

- [ ] Existing similar implementations
- [ ] Relevant aggregates/commands/events
- [ ] API routes and controllers
- [ ] Database schemas and migrations
- [ ] Frontend components and patterns
- [ ] Test patterns for similar features
- [ ] Configuration and environment

**Output format for completeness**:

```
### Completeness

**Areas explored**:
- [x] [Area] - [Brief summary of findings]
- [ ] [Area] - NOT COVERED

**Missing coverage**: [List areas that should have been researched]

**Depth assessment**: [Superficial / Adequate / Thorough]
```

### 5. Summary Quality

| Criterion                                              | Check |
| ------------------------------------------------------ | ----- |
| Summary is accurate representation of findings         |       |
| Key insights are highlighted                           |       |
| Summary is standalone (understandable without details) |       |
| No new information in summary (all backed by details)  |       |

### 6. Open Questions Quality

| Criterion                                   | Check |
| ------------------------------------------- | ----- |
| Unresolved items captured as Open Questions |       |
| Questions are specific (not vague)          |       |
| No critical unknowns left unacknowledged    |       |
| Clear what would answer each question       |       |

### 7. Press Release Alignment (if --press-release provided)

| Criterion                                            | Check |
| ---------------------------------------------------- | ----- |
| Research covers scope defined in Press Release       |       |
| All Press Release "NEEDS RESEARCH" items addressed   |       |
| User stories have supporting research                |       |
| Technical constraints from Press Release validated   |       |
| No major Press Release requirements missing research |       |

**Cross-reference with Press Release to verify research addresses**:

| Press Release Section               | Research Should Cover                          |
| ----------------------------------- | ---------------------------------------------- |
| **Technical Constraints**           | Existing capabilities, new capabilities needed |
| **Open Questions / Needs Research** | Direct answers to listed questions             |
| **Edge Cases**                      | How existing code handles similar cases        |
| **Current State → End State**       | What currently exists vs what needs building   |

**Output format for Press Release coverage**:

```
### Press Release Coverage

**Press Release Questions Addressed**:
- [ ] [Question from Press Release] → [Answered in section X / Not addressed]

**Technical Constraints Explored**:
- [ ] [Dependency] → [Researched / Not covered]

**Coverage gaps**: [List Press Release items that research doesn't address]
```

### 8. Planning Readiness

Verify research surfaces actionable information for planning:

| Criterion                                              | Check |
| ------------------------------------------------------ | ----- |
| Decisions have clear rationale (if present)            |       |
| Open questions are specific and actionable             |       |
| Architecture notes are clear enough to inform planning |       |
| Research identifies what exists vs what needs building |       |

---

## Step 3: Verification Sampling

**Spot-check 3-5 file:line references**:

1. Pick references from different sections
2. Read the actual files to verify accuracy
3. Flag any inaccurate references

```markdown
### Reference Verification

| Reference                          | Actual Content      | Status           |
| ---------------------------------- | ------------------- | ---------------- |
| `path/to/accounts.ex:45`           | [Brief description] | ✅ Accurate      |
| `path/to/tenants/supervisor.ex:23` | [Brief description] | ❌ Line mismatch |
```

---

## Step 4: Present Review Results

Format your review as:

```markdown
# Research Review: [Document Name]

**Document**: [path]
**Reviewed**: [date]
**Press Release Reference**: [path or "Not provided"]

---

## Summary

**Overall**: ✅ Ready for Planning | ⚠️ Minor Issues | ❌ Needs Revision

[1-2 sentence summary of findings]

---

## Checklist Results

### Document Structure

- ✅ All required sections present
- ❌ Missing Code References section

### Reference Quality

- ⚠️ 2 of 5 spot-checked references had stale line numbers

[...continue for all categories...]

---

## Reference Verification

| Reference          | Verified | Notes                       |
| ------------------ | -------- | --------------------------- |
| `path/file.ex:123` | ✅       | Content matches description |
| `path/file.ex:456` | ❌       | Line moved to 478           |

---

## Coverage Analysis

### Covered Areas

- [Area 1] - Thoroughly documented
- [Area 2] - Adequately covered

### Gaps Identified

- [Gap 1] - [Why this matters for planning]
- [Gap 2] - [What's missing]

---

## Issues Requiring Action

### ❌ Critical Issues (must fix)

1. **[Issue Title]**
   - Location: [Section]
   - Problem: [What's wrong]
   - Impact: [Why this blocks planning]

### ⚠️ Warnings (should fix)

1. **[Issue Title]**
   - Concern: [Why this matters]

### 💡 Consider (optional)

1. **[Issue Title]**
   - Suggestion: [Nice-to-have improvement]

---

## Acknowledgment Required

Before proceeding to `/pipe-plan-implementation`, please confirm:

- [ ] I've reviewed all ❌ critical issues and will address them
- [ ] I've considered all ⚠️ warnings
- [ ] I've noted 💡 optional improvements (address now or later)
- [ ] Coverage gaps are acceptable or will be addressed
- [ ] I'm ready to proceed (or will add more research first)

Reply with which items you'll address, or "proceed" to continue.
```

---

## Step 5: Wait for Acknowledgment

**BLOCKING**: Do not suggest next steps until the user has:

1. Acknowledged each critical issue
2. Confirmed coverage is sufficient for planning

If user requests more research:

- Suggest specific follow-up research areas
- Remind them to append to the same document

---

## Review Principles

1. **Press Release drives research** - Research should answer Press Release's questions
2. **Code references are essential** - Every claim needs backing
3. **AS-IS only** - Document what exists, not what should exist
4. **Completeness matters** - Better to explore broadly than miss key areas
5. **Surface decisions** - Research should inform planning, not just describe code

---

## Common Issues to Watch For

### Missing Press Release Coverage

- Technical constraints mentioned in Press Release but not researched
- "Open Questions" section items not addressed
- Edge cases that need codebase investigation

### Code Reference Problems

- Outdated line numbers (code has changed)
- Files that don't exist (typos or deleted)
- References to wrong modules/functions
- Claims without any code backing

### AS-IS Violations

- "This should be refactored..."
- "A better approach would be..."
- "The problem with this is..."
- "We recommend..."

### Incomplete Coverage

- Missing test patterns (how to test similar features)
- Missing frontend analysis for full-stack features
- Missing integration points with other systems

---

## Critical Constraints

- **Verify references**: Spot-check actual files, don't trust blindly
- **Check objectivity**: Flag any opinions masquerading as facts
- **Assess coverage**: Think about what a planner would need to know
- **Blocking mode**: Must get acknowledgment before handoff
- **Be specific**: Every gap must explain why it matters

## Handoff to Planning

Only after user acknowledgment:

```markdown
---

## Next Steps

Your research is ready for planning. Proceed with:

/pipe-plan-implementation --project [project-slug] --ref [ref] --research docs/dev/[project-slug]/[task-slug]/research.md [feature-name]
```

Use the same reference and project slug from the research path, and include the research path.
