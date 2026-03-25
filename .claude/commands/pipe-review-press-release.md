# Review Press Release Document

Review a Press Release + FAQ document for completeness, quality, and alignment with the project plan: $ARGUMENTS

## Purpose

This command validates that a Press Release document:

1. Aligns with the referenced milestone/user story in the project plan
2. Follows the correct structure and quality standards
3. Contains no implementation details (which belong in `/pipe-plan-implementation`)
4. Has complete FAQ coverage with actionable answers
5. Has clear, measurable success criteria

---

## I/O

**Input**: `[--project slug] [press-release-path] [--project-plan path]`
**Reads**: press release + parent project's `project-charter.md` or `project-plan.md`
**Output**: Review report (presented to user, not written to file)

---

## Parsing Arguments

Arguments format: `[--project slug] [press-release-path] [--project-plan path]`

- `--project <slug>`: Project folder — used to find press releases when no direct path given
- `[press-release-path]`: Direct path to the press release to review
- `--project-plan <path>`: Explicit path to project plan (overrides parent lookup)

### Document Resolution

**If a direct `press-release-path` is provided** → use it. Look for `project-charter.md` or `project-plan.md` in the parent project directory.

**If no path but `--project` is provided** → list `press-release.md` files in that project folder, ask user to choose.

**If neither is provided**:

1. Scan `docs/dev/` for existing project folders
2. If folders exist → ask user to **select a project**, then **select a press release**
3. If no folders exist → ask user for the press release path

### Examples

| Command                                                              | Behavior                                         |
| -------------------------------------------------------------------- | ------------------------------------------------ |
| `/pipe-review-press-release docs/dev/oauth/m1-oidc/press-release.md` | Review that PR, auto-find project plan           |
| `/pipe-review-press-release --project oauth`                         | List press releases in `oauth`, ask user to pick |
| `/pipe-review-press-release`                                         | Ask user to select project, then press release   |

---

## Your Role

You are a **quality gate reviewer**. Your job is to:

- Verify the Press Release document meets all structural and content requirements
- Identify gaps, ambiguities, or issues that could cause problems downstream
- Present findings as a blocking checklist the user must acknowledge
- **NOT rewrite the document** — flag issues for the author to fix

---

## Review Process

### Step 1: Load Documents

1. Read `AGENTS.md` for project context
2. Read the Press Release document completely
3. Read the project plan document (provided or auto-discovered)
4. Note the milestone/reference from the Press Release metadata

---

### Step 2: Project Plan Alignment Check

Verify against the project plan:

| Check                      | Pass Criteria                                                         |
| -------------------------- | --------------------------------------------------------------------- |
| **Milestone exists**       | The Press Release's milestone (e.g., M0.4) exists in the project plan |
| **User story traced**      | Press Release maps to a specific user story ID in the milestone       |
| **Scope alignment**        | Press Release scope fits within milestone's intended scope            |
| **Dependencies respected** | Press Release doesn't assume features from future milestones          |

**Output format**:

```
### Project Plan Alignment

- ✅/❌ Milestone: [M0.X] - [exists/not found in plan]
- ✅/❌ User Story: [M0.X.Y] - [traced/not traced]
- ✅/⚠️/❌ Scope: [aligned/scope creep detected]
- ✅/❌ Dependencies: [respected/violation found]

**Issues**: [List any alignment problems]
```

---

### Step 3: Press Release Quality Check

Evaluate the Press Release section:

| Check                 | Pass Criteria                                              |
| --------------------- | ---------------------------------------------------------- |
| **Structure**         | Has For/Who/The/Is a/That/Unlike/Our solution format       |
| **Customer language** | No technical jargon, code terms, or implementation details |
| **Length**            | 1-2 paragraphs max in the description                      |
| **Benefit clarity**   | Benefit is measurable or observable                        |
| **Scope clarity**     | Clear what's IN scope (and implicitly what's NOT)          |

**Questions to answer**:

- Can a customer understand this without technical knowledge?
- Would you actually send this to customers when it ships?
- Is it clear what problem is being solved?

**Output format**:

```
### Press Release Quality

- ✅/❌ Structure: [complete/missing sections]
- ✅/⚠️/❌ Customer language: [pass/technical jargon found]
- ✅/⚠️ Length: [appropriate/too long]
- ✅/⚠️/❌ Benefit clarity: [clear/vague]
- ✅/⚠️/❌ Scope clarity: [clear/ambiguous]

**Issues**: [Quote problematic text and suggest fixes]
```

---

### Step 4: FAQ Completeness Check

Verify all 5 FAQ categories are present and complete:

| Category                   | Required Content                                       |
| -------------------------- | ------------------------------------------------------ |
| **Customer Experience**    | How users discover, use, and interact with the feature |
| **Scope & Non-Goals**      | What's NOT included, minimal version, deferrals        |
| **Technical Constraints**  | Dependencies, new capabilities needed, performance     |
| **Edge Cases & Errors**    | Invalid inputs, system failures, boundary conditions   |
| **Business & Operational** | Success metrics, post-launch risks, compliance         |

For each category, check:

- Questions are relevant to this feature
- Answers are specific (not "TBD" or "depends")
- Edge cases are thoughtfully covered

**Output format**:

```
### FAQ Completeness

| Category               | Present | Questions | Quality         |
| ---------------------- | ------- | --------- | --------------- |
| Customer Experience    | ✅/❌   | [count]   | [complete/gaps] |
| Scope & Non-Goals      | ✅/❌   | [count]   | [complete/gaps] |
| Technical Constraints  | ✅/❌   | [count]   | [complete/gaps] |
| Edge Cases & Errors    | ✅/❌   | [count]   | [complete/gaps] |
| Business & Operational | ✅/❌   | [count]   | [complete/gaps] |

**Missing coverage**: [List topics that should have FAQ entries]
**Vague answers**: [List Q&As that need more specificity]
```

---

### Step 5: Implementation Scope Check

Verify the Implementation Scope section:

| Section                       | Required | Check                                                   |
| ----------------------------- | -------- | ------------------------------------------------------- |
| **Current State → End State** | Yes      | Table with observable changes (not code changes)        |
| **User Stories**              | Yes      | Format: "As a [user], I can [action] so that [benefit]" |
| **Success Criteria**          | Yes      | Observable/testable outcomes                            |
| **Known Compromises**         | Yes      | Documented trade-offs with rationale                    |
| **Open Questions**            | Optional | Flagged items needing research                          |

**Anti-patterns to flag**:

- Technical tasks ("Create file X", "Add function Y")
- Implementation order or task dependencies
- Code examples, SQL, or JSON schemas
- File paths to create/modify
- Any "how to implement" details

**Output format**:

```
### Implementation Scope

- ✅/❌ Current/End State table: [present/missing] - [observable outcomes/code-level]
- ✅/⚠️/❌ User Stories: [count] stories - [correct format/format issues]
- ✅/⚠️/❌ Success Criteria: [count] criteria - [testable/vague]
- ✅/⚠️/❌ Known Compromises: [documented/missing]
- ✅/⚠️ Open Questions: [present/none]

**Anti-patterns found**:
- [Quote any implementation details that don't belong]
```

---

### Step 6: Content Quality Check

Verify the document is complete and ready for handoff:

| Check                    | Pass Criteria                           |
| ------------------------ | --------------------------------------- |
| **No TODO items**        | No "TODO", "TBD", or placeholder text   |
| **No unresolved items**  | No "NEEDS DECISION" or "NEEDS RESEARCH" |
| **Scope constraint**     | Not a kitchen sink of features          |
| **Internal consistency** | No contradictions between sections      |

**Output format**:

```
### Content Quality

- ✅/❌ No TODO/TBD items: [clean/found at: section]
- ✅/❌ No unresolved items: [clean/found at: section]
- ✅/⚠️ Scope constraint: [focused/too broad]
- ✅/❌ Internal consistency: [consistent/contradictions found]

**Issues**: [List any quality problems]
```

---

### Step 7: Traceability Check

Verify everything traces back:

- Every user story traces to the Press Release or an FAQ answer
- Success criteria map to user stories
- No "orphan" requirements that don't connect to customer outcomes

**Output format**:

```
### Traceability

- ✅/❌ All user stories trace to PR/FAQ: [yes/orphans found]
- ✅/❌ Success criteria map to stories: [yes/gaps]

**Orphan items**: [List items that don't trace back]
```

---

## Review Output Format

Generate a structured review report:

```markdown
# Press Release Review: [Document Name]

**Document**: [path to Press Release]
**Reviewed**: [ISO date]
**Project Plan**: [path or "Auto-discovered: [path]" or "Not found"]
**Milestone**: [M0.X]

---

## Summary

**Overall**: ✅ Ready for Research | ⚠️ Minor Issues | ❌ Needs Revision

**Key Findings**:

- [1-3 bullet summary of main issues or strengths]

---

## Detailed Review

### Project Plan Alignment

[Step 2 output]

### Press Release Quality

[Step 3 output]

### FAQ Completeness

[Step 4 output]

### Implementation Scope

[Step 5 output]

### Content Quality

[Step 6 output]

### Traceability

[Step 7 output]

---

## Action Items

### ❌ Must Fix (Blocking)

1. **[Issue Title]**
   - Location: [Section/Line]
   - Problem: [What's wrong]
   - Suggestion: [How to fix]

### ⚠️ Should Fix (Recommended)

1. **[Issue Title]**
   - Location: [Section]
   - Concern: [Why this matters]

### 💡 Consider (Optional)

1. **[Enhancement]**
   - Suggestion: [Nice-to-have improvement]

---

## Acknowledgment Required

Before proceeding to `/pipe-research`, please confirm you've reviewed these issues:

- [ ] I've reviewed all ❌ critical issues and will fix them
- [ ] I've considered all ⚠️ warnings
- [ ] I'm ready to proceed (or will revise first)

Reply with which items you'll address, or "proceed" to continue with current document.
```

---

## Wait for Acknowledgment

**BLOCKING**: Do not suggest next steps until the user has:

1. Acknowledged each critical issue (fixed or accepted)
2. Confirmed they're ready to proceed

If user says "proceed" or "continue":

- Remind them of any unaddressed ❌ critical issues
- Confirm one final time before suggesting `/pipe-research`

If user provides fixes:

- Re-review the specific sections that changed
- Update your assessment

---

## Review Principles

1. **Customer perspective first** - If it's not about customer outcomes, it shouldn't be here
2. **No implementation details** - That's what `/pipe-research` and `/pipe-plan-implementation` are for
3. **Completeness over perfection** - Better to have all sections with some gaps than polished incomplete doc
4. **Trace everything** - Every requirement should connect to customer value
5. **Be specific** - Vague answers indicate unclear thinking

---

## Common Issues to Watch For

### In Press Release

- Technical terms ("API", "database", "schema") instead of customer language
- Features instead of benefits
- Multiple features crammed into one PR (should be separate Press Releases)

### In FAQ

- "TBD" or "depends" answers that need decisions
- Missing edge cases (what if X fails?)
- No discussion of what's NOT in scope

### In Implementation Scope

- Tasks disguised as user stories ("As a developer, I can create the API...")
- Success criteria that require code inspection to verify
- Missing the Current/End State table

---

## Critical Constraints

- **Read completely**: Read the entire document before judging
- **Be specific**: Reference exact sections when flagging issues
- **Be actionable**: Every issue must have a clear path to resolution
- **Blocking mode**: Must get acknowledgment before handoff
- **No rewriting**: Flag issues, don't fix them yourself
- **Objective**: Apply the same standards regardless of document author

---

## Handoff to Research

Only after user acknowledgment:

```markdown
---

## Next Steps

Your Press Release is ready for research. Proceed with:

/pipe-research --project [project-slug] --ref [ref] [feature-name]
```

Use the same reference and project slug from the Press Release path (e.g., reference `m0.3` and project `oauth-system` from `docs/dev/oauth-system/m0.3-tenant-creation/press-release.md`).

---

Start by reading the Press Release and project plan documents.
