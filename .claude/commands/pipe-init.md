# Initialize Agentic Pipeline

Initialize the agentic pipeline for this codebase by analyzing the project and generating AGENTS.md files.

## Purpose

This command:

1. Analyzes the codebase to understand structure, tech stack, and patterns
2. Generates a root AGENTS.md file with project-wide guidelines
3. Identifies subdirectories that would benefit from their own AGENTS.md
4. Generates sub-AGENTS.md files for those directories
5. Updates root AGENTS.md with references to all subdirectory files
6. All interactively, with user approval at each step

---

## Your Role

You are a **codebase analyst and documentation generator**. Your job is to:

- Explore the codebase thoroughly to understand its structure
- Generate accurate, actionable AGENTS.md files
- Follow the exact output format specified below
- Work interactively with the user, getting approval before writing files

---

## Process Overview

```
Phase 1: Analyze Codebase
    ↓
Phase 2: Generate Root AGENTS.md (user approval)
    ↓
Phase 3: Identify Subdirectory AGENTS.md Locations
    ↓
Phase 4: Generate Subdirectory AGENTS.md Files (user approval each)
    ↓
Phase 5: Update Root AGENTS.md with Required Reading (user approval)
    ↓
Complete
```

---

## Output Format

You MUST follow these exact templates when generating AGENTS.md files. Do not deviate from this structure.

### Root AGENTS.md Template

```markdown
# [Project Name] - AI Agent Guidelines

[1-2 sentence project description - AI-focused, not marketing copy]

## Tech Stack

| Component | Technology | Version |
|-----------|------------|---------|
| Language | [e.g., TypeScript] | [version] |
| Framework | [e.g., Next.js] | [version] |
| Database | [e.g., PostgreSQL] | [version] |
| [Other] | [Technology] | [version] |

## Directory Structure

```
project/
├── [dir]/           # [purpose]
│   ├── [subdir]/    # [purpose]
│   └── [subdir]/    # [purpose]
├── [dir]/           # [purpose]
└── [dir]/           # [purpose]
```

## Commands

### Quick Verification

| Command | Purpose |
|---------|---------|
| `[command]` | [what it checks] |
| `[command]` | [what it checks] |

### Full Verification

```bash
[full verification command or chain of commands]
```

### Specific Operations

| Task | Command |
|------|---------|
| Run single test | `[command with placeholder]` |
| Format code | `[command]` |
| Lint | `[command]` |
| [Other task] | `[command]` |

## Required Reading

| Working In | Read First |
|------------|------------|
| `[directory]/` | [`[directory]/AGENTS.md`]([directory]/AGENTS.md) |
| `[directory]/` | [`[directory]/AGENTS.md`]([directory]/AGENTS.md) |

## Boundaries

### Always Do

- [Action that must always be performed]
- [Another required action]

### Ask First

- [Action requiring user confirmation]
- [Another action requiring confirmation]

### Never Do

- [Prohibited action]
- [Another prohibited action]

## Code Conventions

### Naming

| Type | Convention | Example |
|------|------------|---------|
| [Type] | [convention] | `[example]` |
| [Type] | [convention] | `[example]` |

### [Other Convention Category]

- [Convention with example]
- [Another convention]
```

### Subdirectory AGENTS.md Template

```markdown
# [Directory Name] - AI Agent Guidelines

[1 sentence describing what this directory contains]

> For project-wide guidelines, see [root AGENTS.md](../AGENTS.md)

## Overview

[2-3 sentences about purpose and organization of this directory]

## Directory Structure

```
[directory]/
├── [subdir]/    # [purpose]
├── [subdir]/    # [purpose]
└── [file]       # [purpose]
```

## Patterns

### [Pattern Name]

[Brief description]

```[language]
// Example from actual codebase
[code example]
```

### [Another Pattern]

[Brief description with code example]

## Conventions

- [Convention specific to this directory]
- [Another convention]
- [File naming convention if applicable]

## Common Tasks

### [Task Name]

```bash
[command]
```

### [Another Task]

```bash
[command]
```

## Related Files

| File | Relationship |
|------|--------------|
| `[path]` | [how it relates] |
| `[path]` | [how it relates] |

## Gotchas

- [Thing that might trip up an AI agent]
- [Common mistake to avoid]
- [Non-obvious behavior]
```

### Template Rules

1. **All sections are required** unless marked optional or not applicable
2. **Use tables** for structured data (tech stack, commands, conventions)
3. **Use code blocks** for all commands and examples
4. **Include real examples** from the actual codebase, not generic placeholders
5. **Keep descriptions brief** - no paragraphs longer than 3 lines
6. **Commands must be copy-pasteable** - test them if possible

### Length Guidelines

| File Type | Target | Maximum |
|-----------|--------|---------|
| Root AGENTS.md | 200-400 lines | 600 lines |
| Subdirectory AGENTS.md | 50-150 lines | 250 lines |

---

## Phase 1: Analyze Codebase

Explore the codebase to understand its structure, tech stack, and patterns.

### Step 1.1: Directory Structure

Use Glob to map the top-level structure:

- Identify key directories (source, tests, config, docs, etc.)
- Note any monorepo or multi-package structure
- Look for split application patterns (separate backend/frontend)

### Step 1.2: Tech Stack Discovery

Read dependency/config files to identify the tech stack:

| File                             | Indicates           |
| -------------------------------- | ------------------- |
| `mix.exs`                        | Elixir/Phoenix      |
| `package.json`                   | JavaScript/Node.js  |
| `Cargo.toml`                     | Rust                |
| `go.mod`                         | Go                  |
| `requirements.txt`, `pyproject.toml` | Python          |
| `Gemfile`                        | Ruby                |
| `pom.xml`, `build.gradle`        | Java                |
| `*.csproj`                       | .NET                |

Also check for version managers: `.tool-versions`, `.nvmrc`, `.ruby-version`, `.python-version`

### Step 1.3: Architecture Understanding

Sample 3-5 representative source files to understand:

- Code organization patterns (MVC, CQRS, DDD, etc.)
- Module/package structure
- Naming conventions
- Import/dependency patterns

### Step 1.4: Build & Test Commands

Find build/test commands in:

- `Makefile`
- `package.json` scripts
- `mix.exs` aliases
- CI configuration (`.github/workflows/`, `.gitlab-ci.yml`, etc.)

### Step 1.5: Existing Documentation

Check for existing documentation:

- `README.md`
- `AGENTS.md`, `CLAUDE.md`, `COPILOT.md`
- `docs/` directory
- `CONTRIBUTING.md`, `ARCHITECTURE.md`

### Step 1.6: Present Analysis

Present your findings to the user:

```markdown
## Codebase Analysis

### Project Overview
[1-2 sentences describing what this project does]

### Tech Stack
| Component | Technology | Version |
|-----------|------------|---------|
| ... | ... | ... |

### Directory Structure
```
project/
├── [dir]/    # [purpose]
└── [dir]/    # [purpose]
```

### Architecture Pattern
[MVC, CQRS, microservices, etc.]

### Build/Test Commands Found
| Command | Purpose |
|---------|---------|
| ... | ... |

### Existing AGENTS.md Files
[list or "None found"]

---

Proceed with generating AGENTS.md? [Y/n]
```

Wait for user confirmation before proceeding.

---

## Phase 2: Generate Root AGENTS.md

Generate the root AGENTS.md file following the **Root AGENTS.md Template** in the Output Format section above.

### Writing Style

- **Concise over comprehensive** - Tables and lists over paragraphs
- **Actionable over descriptive** - Commands, not explanations
- **Examples over explanations** - Show, don't tell
- **Specific over generic** - No "follow best practices"
- **Copy-pasteable commands** - Must work when pasted

### Voice

- Direct: Use imperative mood ("Run tests" not "You should run tests")
- Neutral: No enthusiasm or marketing language
- Precise: Use exact names, paths, commands

### Don't Include

- Historical context ("We migrated from Rails...")
- Opinions ("Phoenix is great because...")
- Generic advice ("Follow best practices")
- README duplication

### Section Priority (if space limited)

1. Commands (agents need to verify work)
2. Boundaries (agents need to know limits)
3. Tech Stack (agents need context)
4. Directory Structure (agents need to navigate)
5. Conventions (agents need to match style)

### Generate and Present

Generate the AGENTS.md content and present it to the user:

```markdown
## Proposed AGENTS.md

---
[Generated content following Root AGENTS.md Template exactly]
---

Write this to AGENTS.md? [Y/n/edit]
```

**Note**: Leave the "Required Reading" section empty for now. It will be populated in Phase 5 after subdirectory AGENTS.md files are created.

If user approves: Write the file
If user wants to edit: Let them provide feedback, regenerate
If user declines: Skip to Phase 3

---

## Phase 3: Identify Subdirectory AGENTS.md Locations

Based on your analysis, identify directories that would benefit from their own AGENTS.md file.

### Criteria for Subdirectory AGENTS.md

Only recommend directories that meet these criteria:

- Different tech stack or language than root
- Complex patterns worth documenting separately
- Frequently edited areas with specific conventions
- At least 10+ source files
- Would genuinely help an AI agent working in that area

### Present Recommendations

```markdown
## Recommended Subdirectory AGENTS.md Locations

Based on the codebase structure, these directories would benefit from their own AGENTS.md:

1. `backend/` - Elixir/Phoenix backend with specific patterns
2. `frontend/` - React frontend with component conventions
3. `backend/lib/app/contexts/` - Domain contexts with CQRS patterns

Generate sub-AGENTS.md files for these? [Y/n/select]
```

If user selects specific directories, only generate for those.

---

## Phase 4: Generate Subdirectory AGENTS.md Files

For each approved directory, generate a focused AGENTS.md file following the **Subdirectory AGENTS.md Template** in the Output Format section above.

### Generation Process for Each Directory

1. **Explore the directory** - List files, read 2-3 representative files
2. **Identify patterns** - Find patterns specific to this area
3. **Generate content** - Create focused, non-redundant content
4. **Present to user** - Get approval before writing

```markdown
## Proposed: [directory]/AGENTS.md

---
[Generated content]
---

Write this to [directory]/AGENTS.md? [Y/n]
```

### Skip Criteria

If after exploration you find:

- Directory is too simple to warrant its own AGENTS.md
- All relevant info is already in root AGENTS.md
- Directory contains only generated/vendor code

Then skip and explain why.

---

## Phase 5: Update Root AGENTS.md with Required Reading

After all subdirectory AGENTS.md files are created, update the root AGENTS.md to include references to them.

### Update Process

1. Read the current root AGENTS.md
2. Populate the "Required Reading" section with all created subdirectory files
3. Present the updated section to the user for approval

### Present Update

```markdown
## Updating Root AGENTS.md

Adding Required Reading section with references to subdirectory AGENTS.md files:

---
## Required Reading

| Working In | Read First |
|------------|------------|
| `backend/` | [`backend/AGENTS.md`](backend/AGENTS.md) |
| `frontend/` | [`frontend/AGENTS.md`](frontend/AGENTS.md) |
| `tests/` | [`tests/AGENTS.md`](tests/AGENTS.md) |
---

Update AGENTS.md with this section? [Y/n]
```

If user approves: Update the root AGENTS.md file
If user declines: Leave root AGENTS.md as-is

---

## Examples

These examples show what useful subdirectory AGENTS.md files might contain for common project structures.

### Example: Backend API Controllers (`backend/controllers/AGENTS.md`)

```markdown
# Controllers - AI Agent Guidelines

HTTP request handlers that validate input, call services, and format responses.

> For project-wide guidelines, see [root AGENTS.md](../../AGENTS.md)

## Overview

Controllers handle HTTP requests for the REST API. Each controller maps to a resource (users, orders, etc.). Controllers should be thin - validate input, call a service, return response.

## Directory Structure

```
controllers/
├── users_controller.ts      # User CRUD operations
├── orders_controller.ts     # Order management
├── auth_controller.ts       # Authentication endpoints
└── base_controller.ts       # Shared controller utilities
```

## Patterns

### Request Handler Pattern

Every endpoint follows this structure:

```typescript
async create(req: Request, res: Response) {
  // 1. Validate input
  const validated = CreateUserSchema.parse(req.body);

  // 2. Call service
  const user = await this.userService.create(validated);

  // 3. Return response
  return res.status(201).json(user);
}
```

### Error Handling

Errors are caught by middleware. Throw typed errors:

```typescript
throw new NotFoundError('User not found');
throw new ValidationError('Invalid email format');
```

## Conventions

- One controller per resource (e.g., `UsersController` for `/users/*`)
- Method names match HTTP verbs: `index`, `show`, `create`, `update`, `destroy`
- No business logic in controllers - delegate to services
- Use Zod schemas for request validation

## Common Tasks

### Add New Endpoint

1. Add method to controller
2. Add route in `routes/index.ts`
3. Add request validation schema if needed

### Test a Controller

```bash
npm test -- controllers/users_controller.test.ts
```

## Related Files

| File | Relationship |
|------|--------------|
| `services/` | Controllers call services for business logic |
| `schemas/` | Request validation schemas |
| `routes/index.ts` | Route definitions |
| `middleware/error_handler.ts` | Catches thrown errors |

## Gotchas

- Never access database directly - always go through services
- Response format must match OpenAPI spec in `docs/api.yaml`
- Authentication is handled by middleware, not controllers
```

### Example: Frontend React Components (`frontend/components/AGENTS.md`)

```markdown
# Components - AI Agent Guidelines

Reusable React components for the application UI.

> For project-wide guidelines, see [root AGENTS.md](../../AGENTS.md)

## Overview

React components organized by type: layout, common (shared UI), and feature-specific. Uses TypeScript, CSS Modules for styling, and React Query for server state.

## Directory Structure

```
components/
├── layout/          # Page structure (Header, Footer, Sidebar)
├── common/          # Shared UI (Button, Modal, Form inputs)
├── features/        # Feature-specific components
│   ├── users/       # User-related components
│   └── orders/      # Order-related components
└── index.ts         # Public exports
```

## Patterns

### Component Structure

```tsx
// ComponentName.tsx
interface ComponentNameProps {
  title: string;
  onAction?: () => void;
}

export function ComponentName({ title, onAction }: ComponentNameProps) {
  return (
    <div className={styles.container}>
      <h2>{title}</h2>
      {onAction && <Button onClick={onAction}>Action</Button>}
    </div>
  );
}
```

### Styling Pattern

Each component has a co-located CSS Module:

```
Button/
├── Button.tsx
├── Button.module.css
└── Button.test.tsx
```

## Conventions

- One component per file, named export matching filename
- Props interface named `[ComponentName]Props`
- Use CSS Modules, not inline styles or global CSS
- Prefer composition over prop drilling

## Common Tasks

### Create New Component

1. Create folder: `components/common/NewComponent/`
2. Add `NewComponent.tsx`, `NewComponent.module.css`, `NewComponent.test.tsx`
3. Export from `components/index.ts`

### Run Component Tests

```bash
npm test -- components/common/Button
```

### View in Storybook

```bash
npm run storybook
```

## Related Files

| File | Relationship |
|------|--------------|
| `hooks/` | Custom hooks used by components |
| `styles/variables.css` | Design tokens (colors, spacing) |
| `types/` | Shared TypeScript types |

## Gotchas

- Don't import from `features/` into `common/` - causes circular deps
- All text must use `t()` for i18n - no hardcoded strings
- Images go in `public/images/`, not component folders
```

### Example: Tests Directory (`tests/AGENTS.md`)

```markdown
# Tests - AI Agent Guidelines

Test files for the application using Jest and React Testing Library.

> For project-wide guidelines, see [root AGENTS.md](../AGENTS.md)

## Overview

Tests are organized to mirror the source directory structure. Unit tests live alongside source files. Integration and E2E tests live here.

## Directory Structure

```
tests/
├── integration/     # API integration tests
├── e2e/             # End-to-end browser tests
├── fixtures/        # Shared test data
├── helpers/         # Test utilities
└── setup.ts         # Jest setup file
```

## Patterns

### Unit Test Pattern

```typescript
describe('UserService', () => {
  describe('create', () => {
    it('creates user with valid data', async () => {
      const userData = fixtures.validUser();
      const result = await userService.create(userData);
      expect(result.email).toBe(userData.email);
    });

    it('throws ValidationError for invalid email', async () => {
      const userData = fixtures.validUser({ email: 'invalid' });
      await expect(userService.create(userData))
        .rejects.toThrow(ValidationError);
    });
  });
});
```

### Integration Test Pattern

```typescript
describe('POST /api/users', () => {
  it('creates user and returns 201', async () => {
    const response = await request(app)
      .post('/api/users')
      .send(fixtures.validUser());

    expect(response.status).toBe(201);
    expect(response.body).toHaveProperty('id');
  });
});
```

### Fixture Pattern

```typescript
// fixtures/users.ts
export function validUser(overrides = {}) {
  return {
    email: `test-${Date.now()}@example.com`,
    name: 'Test User',
    ...overrides,
  };
}
```

## Conventions

- Test file naming: `[source-file].test.ts`
- One `describe` block per function/method
- Use fixtures for test data, not inline objects
- Clean up test data in `afterEach` hooks

## Common Tasks

### Run All Tests

```bash
npm test
```

### Run Specific Test File

```bash
npm test -- path/to/file.test.ts
```

### Run Tests in Watch Mode

```bash
npm test -- --watch
```

### Run E2E Tests

```bash
npm run test:e2e
```

### Update Snapshots

```bash
npm test -- --updateSnapshot
```

## Related Files

| File | Relationship |
|------|--------------|
| `jest.config.js` | Jest configuration |
| `tests/setup.ts` | Global test setup |
| `.env.test` | Test environment variables |

## Gotchas

- Integration tests use a separate test database - see `.env.test`
- E2E tests require the app to be running: `npm run dev` first
- Snapshot tests are discouraged - prefer explicit assertions
- Mock external APIs in `tests/helpers/mocks/`
```

---

## Completion

After all files are generated and root is updated:

```markdown
## Initialization Complete

### Files Created

| File | Purpose |
|------|---------|
| `AGENTS.md` | Root guidelines with Required Reading references |
| `backend/AGENTS.md` | Backend-specific patterns and conventions |
| `frontend/AGENTS.md` | Frontend-specific patterns and conventions |
| `tests/AGENTS.md` | Testing patterns and conventions |

### File Relationships

```
AGENTS.md (root)
├── References → backend/AGENTS.md
├── References → frontend/AGENTS.md
└── References → tests/AGENTS.md
```

When an AI agent works in a subdirectory, they should:
1. Read root AGENTS.md for project-wide context
2. Read the subdirectory AGENTS.md for specific guidance

### Next Steps

Your agentic pipeline is ready. Available commands:

| Command | Purpose |
|---------|---------|
| `/pipe-research [topic]` | Research codebase questions |
| `/pipe-plan-implementation [feature]` | Plan feature implementation |
| `/pipe-implement [plan-path]` | Execute implementation plan |
| `/pipe-bugfix [issue]` | Fix bugs systematically |

Start with `/pipe-research` to explore the codebase, or `/pipe-plan-implementation` to plan a feature.
```

---

## Critical Constraints

- **Follow output format exactly**: Use the templates in Output Format section verbatim
- **Accuracy first**: Only document what you actually find, never assume
- **User approval**: Always get approval before writing files
- **Update root with references**: After creating subdirectory files, always update root AGENTS.md with Required Reading section
- **No duplication**: Subdirectory AGENTS.md files should not repeat root content
- **Actionable content**: Every section should help an AI agent work effectively
- **Copy-pasteable**: All commands must work when pasted
- **Concise**: Respect length guidelines

---

Start by exploring the codebase structure and tech stack.
