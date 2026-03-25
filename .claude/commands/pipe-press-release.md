# Create Press Release + FAQ Document

Help me create a Press Release + FAQ document using the "Working Backwards" method - start with the customer outcome, then derive what needs to be built.

## I/O

**Input**: `[--project slug] [--ref id] [feature-description]`
**Output**: `docs/dev/[project-slug]/[task-slug]/press-release.md`
**Commit**:
```
[project-slug/task-slug] press-release: <short summary>

<description of the feature, target users, and key outcomes>
```

---

## Parsing Arguments

Arguments format: `[--project slug] [--ref id] [feature-description]`

- `--project <slug>`: Project folder name under `docs/dev/`
- `--ref <id>`: Optional. Milestone (`m1`, `M0.3`), ticket (`PROJ-123`), or issue (`#42`)
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

- With reference: `[ref-lowercase]-[description-kebab-case]` (e.g., `m1-tenant-creation`)
- Without reference: `[description-kebab-case]` (e.g., `tenant-creation`)

Rules: lowercase, kebab-case, max 4 words for description.

### Examples

| Command                                                            | Project      | Task Slug            | Output                                                   |
| ------------------------------------------------------------------ | ------------ | -------------------- | -------------------------------------------------------- |
| `/pipe-press-release --project workforce --ref m1 tenant creation` | `workforce`  | `m1-tenant-creation` | `docs/dev/workforce/m1-tenant-creation/press-release.md` |
| `/pipe-press-release --project oauth user auth`                    | `oauth`      | `user-auth`          | `docs/dev/oauth/user-auth/press-release.md`              |
| `/pipe-press-release --ref PROJ-123 billing` (projects exist)      | user selects | `proj-123-billing`   | `docs/dev/[selected]/proj-123-billing/press-release.md`  |

---

## Press Release Template

Write in plain language as if announcing to customers. Keep it to 1-2 paragraphs max.

```
### [Feature Name]

**For** [target user/customer segment]
**Who** [have this problem / need this capability]
**The** [feature name]
**Is a** [category - e.g., "new capability", "improvement to X"]
**That** [key benefit in customer terms]
**Unlike** [current state / alternative / workaround]
**Our solution** [primary differentiator / why this approach]

---

[1-2 paragraph description written as if the feature just launched. Describe what customers can now do that they couldn't before. Use concrete examples. Avoid implementation details - focus on outcomes.]
```

### Press Release Quality Checks

- Can a customer understand this without technical knowledge?
- Is the benefit measurable or observable?
- Is it clear what's IN scope (and implicitly what's NOT)?
- Would you actually send this to customers when it ships?

---

## FAQ Section

Build FAQs across these categories. Not all will apply to every feature.

### Customer Experience FAQs

- How does a user discover/access this feature?
- What does the happy path look like step-by-step?
- What happens when [common edge case]?
- What error states exist and how are they communicated?
- How does this interact with existing features the user already uses?

### Scope & Non-Goals FAQs

- What is explicitly NOT included in V1?
- What's the minimal version that still delivers the core benefit?
- What would make us cut scope further if needed?
- What are we deferring to future iterations?

### Technical Constraint FAQs

- What existing system capabilities does this depend on?
- What new capabilities need to be built?
- Are there performance, scale, or data constraints?
- What's the simplest implementation that delivers the PR outcome?

### Edge Cases & Error States FAQs

- What happens if [input is missing/invalid]?
- What happens if [dependent system fails]?
- What happens if [user is in unexpected state]?
- What are the boundary conditions?

### Business & Operational FAQs

- How will we know if this is successful?
- What could go wrong post-launch?
- Does this require changes to support/ops processes?
- Are there compliance or security considerations?

---

## From PR FAQ to Implementation

The PR FAQ defines WHAT to build and WHY. The HOW is determined later through the `/pipe-research` → `/pipe-plan-implementation` → `/pipe-implement` workflow.

### What Belongs in PR FAQ

- **Current State → End State**: Observable changes from user/stakeholder perspective (not code changes)
  - Current: "Developers must manually set up PostgreSQL and run migrations"
  - End: "Developers run `docker-compose up` and the database is ready"
- **User Stories**: Discrete user capabilities derived from the PR
- **Success Criteria**: How we know the feature is working
- **Known Constraints**: Technical or business constraints discovered in FAQ
- **Known Compromises**: Intentional deferrals or shortcuts we're accepting (tech debt we acknowledge)
- **Open Questions**: Things that need research before implementation

### What Does NOT Belong in PR FAQ

- **Technical Tasks**: No "Create file X" or "Add function Y"
- **Implementation Order**: No task dependency diagrams
- **Code Examples**: No SQL, JSON schemas, or code blocks
- **File Paths**: No references to specific files to create/modify

### Why This Separation?

1. **PR FAQs stay stable** - They describe outcomes, which don't change as implementation evolves
2. **Fresh research** - The `/pipe-research` command explores the actual codebase, not stale assumptions
3. **Better plans** - `/pipe-plan-implementation` creates implementation details based on current code state
4. **Single source of truth** - Implementation details live in plan files, not scattered across docs

### User Story Format

From the PR and FAQ answers, identify discrete user capabilities:

```
As a [user type], I can [action] so that [benefit from PR]
```

Each story should trace back to:

- A claim in the Press Release, OR
- An answer in the FAQ

If a story doesn't trace back, question whether it's in scope.

**Note**: User stories become comprehensive E2E test scenarios during the Final Verification phase of implementation. Write them as testable user journeys.

---

## Process

### Phase 1: Gather Requirements

If I haven't described a feature yet, ask me to describe the problem or feature. Ask clarifying questions to understand:

- Who is the target user?
- What problem are they facing today?
- What does success look like for them?

### Phase 2: Draft PR and Write to File

Once clear on the feature, draft a Press Release using the template. **Write it to the file immediately** so I can review and edit it directly. Then present it and ask:

- "Does this capture what you're trying to achieve?"
- "Is there anything here that's NOT in scope?"
- "Is there anything missing from the customer benefit?"

### Phase 3: Build FAQ

Work through FAQ categories systematically. For each relevant question:

- Propose an answer if you can infer it
- Flag as "NEEDS DECISION" if it requires user input
- Flag as "NEEDS RESEARCH" if it requires investigation

Update the file as you go.

### Phase 4: Validate Scope

Review the complete PR FAQ and confirm:

- The PR still accurately describes the end goal
- All critical FAQs have answers (not "TBD")
- Scope is constrained to what's essential for the PR benefit

### Phase 5: Finalize Implementation Scope

Only after PR FAQ is validated:

- List user stories derived from the PR and FAQ
- Define success criteria (observable outcomes)
- Flag open questions that need research
- Do NOT specify technical tasks — that happens in `/pipe-plan-implementation`

Update the file with final content.

### Phase 6: Commit

Once the document is finalized, commit following the format in the `## I/O` section at the top of this file.

**Important**: Always write to the file first, then present. This lets me review and make manual edits between steps if needed.

---

## Output Format

Structure the final document as:

```markdown
# [Feature Name] - PR FAQ

> **Milestone**: [M0, M1, etc.] > **Created**: [Date] > **Status**: Draft | Ready for Implementation

---

## Press Release

[The PR content]

## FAQ

### Customer Experience

Q: [Question]
A: [Answer]

### Scope & Non-Goals

Q: [Question]
A: [Answer]

[...other categories...]

## Implementation Scope

### Current State → End State

| Current State               | End State                          |
| --------------------------- | ---------------------------------- |
| [What users experience now] | [What users will experience after] |

### User Stories

- [ ] As a [user], I can [action] so that [benefit]
- [ ] Story 2...

### Success Criteria

- [ ] [Observable outcome 1]
- [ ] [Observable outcome 2]

### Known Compromises

Things we're intentionally deferring or accepting as shortcuts:

- [ ] [Compromise 1 - why we're accepting it]
- [ ] [Compromise 2 - why we're accepting it]

### Open Questions / Needs Research

- [ ] Question 1
- [ ] Question 2

---

## Next Steps

When ready to implement:

1. Run `/pipe-research --project [project-slug] --ref [ref] [feature]` to analyze codebase
2. Run `/pipe-plan-implementation --project [project-slug] --ref [ref] [feature]` to create implementation plan
3. Run `/pipe-implement docs/dev/[project-slug]/[task-slug]/implementation-plan.md` to execute the plan
```

---

## Key Principles

1. **Customer language first** - If you can't explain it simply, you don't understand it yet
2. **Scope is the release valve** - Trade scope for speed and quality, not the reverse
3. **FAQs are pre-mortems** - Every FAQ answered now is a problem avoided later
4. **Living document** - Add questions as they arise, the PR FAQ grows with understanding
5. **Trace everything back** - Every implementation task should connect to the PR or FAQ

---

Start by asking me what feature or problem I'd like to work on.
