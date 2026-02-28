---
name: skill-creator
description: Create new skills for Claude Code. Generates skill directories and creates SKILL.md files with proper frontmatter and structure.
argument-hint: [skill-name] [description] [--disable-model-invocation=false] [--user-invocable=true] [--allowed-tools=""] [--context=inline]
allowed-tools: Write, Bash
---

# Skill Creator

Create new Claude Code skills with proper structure and configuration.

## Usage

```
/skill-creator skill-name "description here" [--flags...]
```

### Examples

```
/skill-creator explain-code "Explain code with diagrams"
/skill-creator deploy "Deploy to production" --disable-model-invocation=true
/skill-readonly "Read files only" --allowed-tools="Read,Grep"
/skill-branch "Run in subagent" --context=fork --agent=Explore
```

## Command Line Options

| Option | Default | Description |
|--------|---------|-------------|
| `skill-name` | Required | Name of the skill (lowercase, hyphens only) |
| `description` | Required | What the skill does and when to use it |
| `--disable-model-invocation` | false | Prevent Claude from automatically invoking this skill |
| `--user-invocable` | true | Show in the / menu for users to invoke directly |
| `--allowed-tools` | "" | Tools Claude can use when skill is active (e.g., "Read,Write,Bash") |
| `--context` | inline | Execution context: `inline`, `fork`, or `auto` |
| `--agent` | general-purpose | When context=fork, which agent type to use |

## Instructions

When this skill is invoked, parse `$ARGUMENTS` and create the skill:

### Step 1: Parse Arguments

Extract from `$ARGUMENTS`:
- First positional arg: skill name
- Second positional arg (quoted): description
- Optional flags: `--disable-model-invocation`, `--user-invocable`, `--allowed-tools`, `--context`, `--agent`

### Step 2: Create Skill Directory

Create: `.claude/skills/[skill-name]/`

### Step 3: Generate SKILL.md

Create `.claude/skills/[skill-name]/SKILL.md` with:

```yaml
---
name: [skill-name]
description: [description]
disable-model-invocation: [true/false]
user-invocable: [true/false]
allowed-tools: "[tools]"
context: [inline/fork]
agent: [agent-type]
---

# [Skill Name Title]

[Instructions for what the skill does]

## Usage
/$ARGUMENTS[0] [arguments]
```

### Step 4: Create Supporting Files

- Create `example.md` with example usage
- Create `reference.md` with detailed documentation
- Create `scripts/` directory for any utility scripts

### Step 5: Confirm and Report

Report back with:
- ✅ Skill created: `.claude/skills/[skill-name]/`
- 📝 Next steps for customizing the skill

## Example Skills to Create

### Simple Command Skill
```
/skill-creator git-status "Show git status"
```

### Read-Only Skill
```
/skill-creator safe-read "Read files only" --allowed-tools="Read,Grep,Glob"
```

### Manual-Only Skill
```
/skill-creator deploy "Deploy to production" --disable-model-invocation=true
```

### Subagent Skill
```
/skill-creator deep-search "Search codebase thoroughly" --context=fork --agent=Explore
```

## After Creating

The user should:
1. Edit the generated `SKILL.md` to add detailed instructions
2. Add example outputs to `example.md`
3. Document any complex behavior in `reference.md`
4. Test the skill with `/[skill-name]`
