# Review Implementation Plan

Review an implementation plan for completeness, technical accuracy, and alignment with research and Press Release: $ARGUMENTS

## Purpose

This command validates that an implementation plan:

1. Delivers all user stories and success criteria from the Press Release
2. Uses patterns and approaches discovered in research
3. Has clear, unambiguous phases with testable success criteria
4. Follows existing codebase patterns and conventions
5. Is concise (under 600 lines) and focused on structure, not implementation code

## I/O

**Input**: `[--project slug] [plan-path] [--press-release path] [--research path]`
**Reads**: plan + sibling `press-release.md` and `research.md` in same task directory
**Output**: Review report (presented to user, not written to file)

---

## Parsing Arguments

Arguments format: `[--project slug] [plan-path] [--press-release path] [--research path]`

- `--project <slug>`: Project folder — used to find plans when no direct path given
- `[plan-path]`: Direct path to the implementation plan to review
- `--press-release <path>`: Explicit path to related press release (overrides sibling lookup)
- `--research <path>`: Explicit path to related research (overrides sibling lookup)

### Document Resolution

**If a direct `plan-path` is provided** → use it. Look for sibling `press-release.md` and `research.md` in the same directory.

**If no path but `--project` is provided** → list `implementation-plan.md` files in that project folder, ask user to choose.

**If neither is provided**:

1. Scan `docs/dev/` for existing project folders
2. If folders exist → ask user to **select a project**, then **select a plan**
3. If no folders exist → ask user for the plan path

### Examples

| Command                                                           | Behavior                                |
| ----------------------------------------------------------------- | --------------------------------------- |
| `/pipe-review-plan docs/dev/oauth/m1-oidc/implementation-plan.md` | Review that plan, auto-find siblings    |
| `/pipe-review-plan --project oauth`                               | List plans in `oauth`, ask user to pick |
| `/pipe-review-plan`                                               | Ask user to select project, then plan   |

## Your Role

You are a **quality gate reviewer**. Your job is to:

- Verify the plan is executable without guesswork
- Ensure phases are atomic and independently verifiable
- Check that all decisions are made (no ambiguity)
- Validate alignment with research findings
- Confirm scope matches Press Release requirements
- Present findings as a blocking checklist

## Review Process

### Step 1: Load Documents

1. Read the plan document completely
2. Read the related Press Release document completely (found or provided)
3. Read the related research document completely (found or provided)
4. Read `AGENTS.md` for project conventions and verification commands

### Step 2: Execute Review Checklist

Work through each category below. For each criterion:

- **✅ PASS**: Criterion fully met
- **⚠️ WARN**: Minor issue, but acceptable
- **❌ FAIL**: Critical issue that must be fixed

---

## Review Checklist

### 1. Document Structure

| Criterion                                                             | Check |
| --------------------------------------------------------------------- | ----- |
| Has YAML frontmatter (date, author, status, branch, related_research) |       |
| Status is `draft` or `approved` (not empty)                           |       |
| Has Overview section                                                  |       |
| Has Architecture Decisions section                                    |       |
| Has Prerequisites section                                             |       |
| Has numbered Phases                                                   |       |
| Has Rollback Plan                                                     |       |
| Has Open Questions section                                            |       |
| Total length under 600 lines                                          |       |

### 2. Architecture Decisions

| Criterion                                       | Check |
| ----------------------------------------------- | ----- |
| All major decisions documented                  |       |
| Each decision has rationale ("because...")      |       |
| No "TBD" or "to be decided" items               |       |
| Decisions align with existing codebase patterns |       |
| Trade-offs explicitly acknowledged              |       |

### 3. Phase Structure (check each phase)

| Criterion                                    | Check |
| -------------------------------------------- | ----- |
| Each phase has clear Goal statement          |       |
| Each phase has Files to Modify list          |       |
| File paths are specific (not vague)          |       |
| Each phase has Module Design section         |       |
| Function signatures include return types     |       |
| Each phase has Success Criteria              |       |
| Phases are atomic (independently verifiable) |       |
| Phase dependencies are clear                 |       |

### 4. Module Design Quality

| Criterion                                       | Check |
| ----------------------------------------------- | ----- |
| Describes structure, not implementation         |       |
| No full function bodies (just signatures)       |       |
| No complete file contents                       |       |
| No boilerplate code                             |       |
| Data structures show fields, not implementation |       |
| Integration points clearly identified           |       |

### 5. Success Criteria Quality

| Criterion                                   | Check |
| ------------------------------------------- | ----- |
| Automated verification references AGENTS.md |       |
| Specific commands to run are clear          |       |
| Manual verification steps are concrete      |       |
| Verification is actually testable           |       |
| Each phase can be verified independently    |       |
| Final phase has end-to-end verification     |       |

### 6. File Path Validation

**Spot-check file paths mentioned in the plan**:

- For existing files: verify they exist
- For new files: verify parent directory exists
- For modifications: check file structure matches expectations

### 7. Press Release Delivery Check

Verify the plan delivers everything promised in Press Release:

| Press Release Section   | Plan Should Deliver                       |
| ----------------------- | ----------------------------------------- |
| **User Stories**        | Each story achievable through plan phases |
| **Success Criteria**    | Each criterion has verification in plan   |
| **Current → End State** | Plan transforms current to end state      |
| **Known Compromises**   | Plan respects listed compromises          |

| Criterion                                         | Check |
| ------------------------------------------------- | ----- |
| Plan covers all user stories from Press Release   |       |
| Success criteria map to Press Release outcomes    |       |
| No scope creep beyond Press Release               |       |
| Known compromises from Press Release acknowledged |       |
| Plan addresses Press Release's open questions     |       |

**Detailed Press Release Analysis Output**:

```
**User Stories Coverage**:
| Story | Phase | Delivered |
|-------|-------|-----------|
| As a [user], I can [action]... | Phase X | [Yes/Partial/No] |

**Success Criteria Mapping**:
| Criterion | Verification | Phase |
|-----------|--------------|-------|
| [Criterion from Press Release] | [How plan verifies] | Phase X |

**Coverage gaps**: [List Press Release items not addressed by plan]
**Over-scope**: [List plan items beyond Press Release scope]
```

### 8. Research Alignment Check

Verify the plan uses findings from research:

| Check                        | Pass Criteria                                 |
| ---------------------------- | --------------------------------------------- |
| **Patterns followed**        | Uses patterns identified in research          |
| **Code references used**     | Plan references research's code locations     |
| **Decisions respected**      | Resolved decisions from research are followed |
| **Open questions addressed** | Research's open questions resolved in plan    |

| Criterion                                      | Check |
| ---------------------------------------------- | ----- |
| Plan references research findings              |       |
| File paths match research documentation        |       |
| Patterns used match researched patterns        |       |
| No contradictions with research findings       |       |
| Open questions from research addressed in plan |       |
| Architecture aligns with researched codebase   |       |

**Detailed Research Analysis Output**:

```
**Patterns from research**:
- [ ] [Pattern] → [Used in Phase X / Not used]

**Research decisions**:
- [ ] [Decision] → [Followed / Deviated - reason?]

**Research open questions**:
- [ ] [Question] → [Resolved in plan / Still open]

**Alignment issues**: [List deviations from research findings]
```

### 9. Conciseness Check

Verify plan is focused and not bloated:

| Check                       | Pass Criteria                      |
| --------------------------- | ---------------------------------- |
| **Line count**              | Under 600 lines                    |
| **No full implementations** | No complete function bodies        |
| **No test implementations** | No full test files                 |
| **No boilerplate**          | No code that will be written again |
| **Structure over code**     | Describes WHAT, not HOW to code it |

**Anti-patterns to flag**:

- Full function implementations (more than signatures)
- Complete file contents
- Test file implementations
- Boilerplate code blocks
- Comments explaining obvious code

**Conciseness Analysis Output**:

```
- **Line count**: [X] lines ([under 600/over 600])
- [ ] No full implementations: [pass/violations]
- [ ] No test implementations: [pass/violations]
- [ ] No boilerplate: [pass/violations]
- [ ] Structure focus: [pass/too much code]

**Bloat found**:
- [Section]: [Description of excess detail]
```

### 10. Ambiguity Check

Verify plan leaves no decisions for implementation time:

**Red flags to detect**:

- "TBD" or "to be determined"
- "Either X or Y" without decision
- "Depending on..." without resolution
- "May need to..." without commitment
- Vague descriptions ("handle appropriately", "as needed")

**Each decision should have**:

- Clear choice made
- Rationale documented
- No alternatives left open

| Criterion                     | Check |
| ----------------------------- | ----- |
| No TBDs or "to be determined" |       |
| No open alternatives          |       |
| No vague language             |       |
| All decisions have rationale  |       |
| All ambiguous terms defined   |       |

**Ambiguity Analysis Output**:

```
- [ ] No TBDs: [pass/found]
- [ ] No open alternatives: [pass/found]
- [ ] No vague language: [pass/found]
- [ ] All decisions documented: [pass/gaps]

**Ambiguous items**:
- "[Quote]" → Needs decision: [What needs to be decided]
```

### 11. Technical Accuracy Check

Verify plan is technically sound:

**File path verification**:

- Existing files: Verify they exist in codebase
- New files: Verify parent directories exist and naming follows conventions

**Pattern compliance**:

- Module names follow project conventions
- Function signatures match existing patterns
- Data structures align with existing schemas

**Integration points**:

- Referenced modules exist
- API routes follow existing patterns
- Database operations use correct patterns (schema prefix, etc.)

**Technical Accuracy Output**:

```
**File paths**:
| Path | Type | Valid |
|------|------|-------|
| `path/to/file.ex` | Existing | [Y/N] |
| `path/to/new_file.ex` | New | [Dir exists/Invalid] |

**Pattern compliance**:
- [ ] Module naming: [follows conventions/issues]
- [ ] Function signatures: [consistent/inconsistent]
- [ ] Data structures: [aligned/misaligned]

**Integration accuracy**:
- [ ] Referenced modules exist: [yes/missing]
- [ ] API patterns followed: [yes/deviations]
- [ ] Database patterns correct: [yes/issues]

**Technical issues**: [List specific problems]
```

### 12. Executability Assessment

| Criterion                                           | Check |
| --------------------------------------------------- | ----- |
| Could an implementer follow this without questions? |       |
| All ambiguous terms defined                         |       |
| No missing steps between phases                     |       |
| Error handling approach specified                   |       |
| Test strategy clear                                 |       |

---

## Step 3: File Path Verification

**Verify 3-5 file paths from the plan**:

```markdown
### File Path Verification

| Path                               | Type   | Status | Notes                          |
| ---------------------------------- | ------ | ------ | ------------------------------ |
| `path/to/accounts/user.ex`         | Exists | ✅     | File exists, structure matches |
| `path/to/tenants/admin.ex`         | New    | ✅     | Parent directory exists        |
| `path/to/events/tenant_created.ex` | Modify | ❌     | File doesn't exist             |
```

---

## Step 4: Present Review Results

Format your review as:

```markdown
# Plan Review: [Document Name]

**Reviewed**: [ISO date]
**Document**: [path to plan]
**Related Press Release**: [path to Press Release]
**Related Research**: [path to research]
**Reference**: [M0.X or topic]

---

## Summary

**Overall Assessment**: ✅ Ready for Implementation | ⚠️ Minor Issues | ❌ Needs Revision

**Key Findings**:

- [1-3 bullet summary of main issues or strengths]

**Line Count**: [X] lines (target: <600)

---

## Detailed Review

### Document Structure

| Section                | Present | Quality                            |
| ---------------------- | ------- | ---------------------------------- |
| YAML Frontmatter       | [Y/N]   | [complete/missing fields]          |
| Overview               | [Y/N]   | [clear/vague]                      |
| Architecture Decisions | [Y/N]   | [with rationale/missing rationale] |
| Prerequisites          | [Y/N]   | [actionable/vague]                 |
| Phases                 | [count] | [well-structured/issues]           |
| Final Verification     | [Y/N]   | [comprehensive/incomplete]         |
| Rollback Plan          | [Y/N]   | [actionable/vague]                 |
| Open Questions         | [Y/N]   | [documented/none]                  |

**Structure issues**:

- [List any missing or problematic sections]

### Press Release Delivery

**User Stories Coverage**:
| Story | Phase | Delivered |
|-------|-------|-----------|
| As a [user], I can [action]... | Phase X | [Yes/Partial/No] |

**Success Criteria Mapping**:
| Criterion | Verification | Phase |
|-----------|--------------|-------|
| [Criterion from Press Release] | [How plan verifies] | Phase X |

**Coverage gaps**: [List Press Release items not addressed by plan]
**Over-scope**: [List plan items beyond Press Release scope]

### Research Alignment

**Patterns from research**:

- [ ] [Pattern] → [Used in Phase X / Not used]

**Research decisions**:

- [ ] [Decision] → [Followed / Deviated - reason?]

**Research open questions**:

- [ ] [Question] → [Resolved in plan / Still open]

**Alignment issues**: [List deviations from research findings]

### Conciseness

- **Line count**: [X] lines ([under 600/over 600])
- [ ] No full implementations: [pass/violations]
- [ ] No test implementations: [pass/violations]
- [ ] No boilerplate: [pass/violations]
- [ ] Structure focus: [pass/too much code]

**Bloat found**:

- [Section]: [Description of excess detail]

### Ambiguity Check

- [ ] No TBDs: [pass/found]
- [ ] No open alternatives: [pass/found]
- [ ] No vague language: [pass/found]
- [ ] All decisions documented: [pass/gaps]

**Ambiguous items**:

- "[Quote]" → Needs decision: [What needs to be decided]

### Technical Accuracy

**File paths**:
| Path | Type | Valid |
|------|------|-------|
| `path/to/file.ex` | Existing | [Y/N] |
| `path/to/new_file.ex` | New | [Dir exists/Invalid] |

**Pattern compliance**:

- [ ] Module naming: [follows conventions/issues]
- [ ] Function signatures: [consistent/inconsistent]
- [ ] Data structures: [aligned/misaligned]

**Integration accuracy**:

- [ ] Referenced modules exist: [yes/missing]
- [ ] API patterns followed: [yes/deviations]
- [ ] Database patterns correct: [yes/issues]

**Technical issues**: [List specific problems]

### Success Criteria Quality

| Phase   | Automated | Manual  | Quality          |
| ------- | --------- | ------- | ---------------- |
| Phase 1 | [Y/N]     | [count] | [testable/vague] |
| Phase 2 | [Y/N]     | [count] | [testable/vague] |

**Vague criteria**:

- Phase [X]: "[Criterion]" → Should be: "[Specific version]"

**Missing criteria**:

- Phase [X]: [What's not verified]

---

## Phase-by-Phase Analysis

### Phase 1: [Name]

- Goal: ✅ Clear
- Files: ✅ All paths verified
- Module Design: ✅ Signatures defined
- Success Criteria: ⚠️ Manual verification could be more specific

### Phase 2: [Name]

- Goal: ✅ Clear
- Files: ❌ `path/to/file.ex` doesn't exist
- Module Design: ✅ Complete
- Success Criteria: ✅ Testable

[...continue for all phases...]

---

## Action Items

### ❌ Must Fix (Blocking)

1. **[Issue Title]**
   - Location: [Phase/Section]
   - Problem: [What's wrong]
   - Impact: [Why this blocks implementation]
   - Suggestion: [How to fix]

### ⚠️ Should Fix (Recommended)

1. **[Issue Title]**
   - Location: [Phase/Section]
   - Concern: [Why this matters]

### 💡 Consider (Optional)

1. **[Issue Title]**
   - Note: [Nice-to-have improvement]

---

## Executability Assessment

**Can this plan be implemented without questions?**

- [Strength 1]
- [Strength 2]
- [Gap that might cause implementer confusion]

---

## Acknowledgment Required

Before proceeding to `/pipe-implement`, please confirm:

- [ ] I've reviewed all ❌ critical issues and will fix them
- [ ] I've considered all ⚠️ warnings
- [ ] Plan is approved and ready for implementation
- [ ] Status has been updated to `approved` in frontmatter

Reply with which items you'll address, or "proceed" to implement.
```

---

## Step 5: Wait for Acknowledgment

**BLOCKING**: Do not suggest implementation until the user has:

1. Acknowledged each critical issue
2. Confirmed the plan is approved
3. Updated status to `approved` (or confirmed it's already approved)

If user makes revisions:

- Re-review specific sections that changed
- Verify fixes address the flagged issues

---

## Review Principles

1. **Press Release is the contract** - Plan must deliver what Press Release promises
2. **Research informs, plan decides** - Use research findings, make clear decisions
3. **No ambiguity** - Every decision made upfront
4. **Concise over complete** - Structure, not code
5. **Testable phases** - If you can't verify it, don't claim it

---

## Common Issues to Watch For

### Press Release Gaps

- User stories not mapped to phases
- Success criteria without verification steps
- Scope creep beyond Press Release boundaries

### Research Deviations

- Using different patterns than research identified
- Ignoring resolved decisions from research
- Not addressing research's open questions

### Structure Problems

- Missing phase goals
- No files listed for modifications
- Module designs without function signatures
- Missing integration descriptions

### Bloat

- Full function implementations (keep to signatures)
- Complete test files (just describe what to test)
- Boilerplate code blocks
- Over-explained obvious patterns

### Ambiguity

- "Depending on X" without committing
- Multiple options without choosing
- Vague success criteria ("feature works")
- Missing rationale for decisions

---

## Critical Constraints

- **Verify paths**: Check that files exist or can be created
- **Check for ambiguity**: Flag anything that would cause an implementer to pause and ask questions
- **Validate scope**: Ensure plan matches Press Release and research
- **Blocking mode**: Must get acknowledgment before handoff
- **Line count**: Flag if plan exceeds 600 lines (too detailed)
- **No implementation code**: Flag if plan contains full function bodies

---

## Handoff to Implementation

Only after user acknowledgment and status is `approved`:

```markdown
---

## Next Steps

Your plan is approved and ready for implementation. Proceed with:

/pipe-implement docs/dev/[project-slug]/[task-slug]/implementation-plan.md
```

Include the actual path to the plan file.

---

Start by reading the plan, Press Release, and research documents.
