#!/bin/bash
# Skill Creator Helper Script

set -e

# Default values
DISABLE_MODEL_INVOCATION="false"
USER_INVOCABLE="true"
ALLOWED_TOOLS=""
CONTEXT="inline"
AGENT="general-purpose"

# Parse arguments
POSITIONAL=()
while [[ $# -gt 0 ]]; do
    case $1 in
        --disable-model-invocation)
            DISABLE_MODEL_INVOCATION="true"
            shift
            ;;
        --user-invocable)
            USER_INVOCABLE="true"
            shift
            ;;
        --no-user-invocable)
            USER_INVOCABLE="false"
            shift
            ;;
        --allowed-tools)
            ALLOWED_TOOLS="$2"
            shift 2
            ;;
        --context)
            CONTEXT="$2"
            shift 2
            ;;
        --agent)
            AGENT="$2"
            shift 2
            ;;
        -*|--*)
            echo "Unknown option $1"
            exit 1
            ;;
        *)
            POSITIONAL+=("$1")
            shift
            ;;
    esac
done

set -- "${POSITIONAL[@]}"

# Check if skill name and description are provided
if [[ -z "$1" ]]; then
    echo "Error: Skill name is required"
    echo "Usage: create-skill.sh <skill-name> [description] [options...]"
    exit 1
fi

SKILL_NAME="$1"
DESCRIPTION="$2"

if [[ -z "$DESCRIPTION" ]]; then
    echo "Error: Description is required"
    echo "Usage: create-skill.sh <skill-name> [description] [options...]"
    exit 1
fi

# Create skill directory
SKILL_DIR="$HOME/.claude/skills/$SKILL_NAME"
mkdir -p "$SKILL_DIR/scripts"

# Generate SKILL.md
cat > "$SKILL_DIR/SKILL.md" << EOF
---
name: $SKILL_NAME
description: $DESCRIPTION
disable-model-invocation: $DISABLE_MODEL_INVOCATION
user-invocable: $USER_INVOCABLE
allowed-tools: "$ALLOWED_TOOLS"
context: $CONTEXT
agent: $AGENT
---

# $SKILL_NAME

EOF

# Add context-specific content
if [[ "$CONTEXT" == "fork" ]]; then
    cat >> "$SKILL_DIR/SKILL.md" << EOF
This skill runs in a subagent context with $AGENT agent.

## Usage
EOF
else
    cat >> "$SKILL_DIR/SKILL.md" << EOF
This skill can be invoked directly with /$SKILL_NAME.

## Usage
EOF
fi

if [[ -n "$ALLOWED_TOOLS" ]]; then
    cat >> "$SKILL_DIR/SKILL.md" << EOF
Allowed tools: $ALLOWED_TOOLS

## Instructions
EOF
else
    cat >> "$SKILL_DIR/SKILL.md" << EOF

## Instructions
EOF
fi

# Create example.md
cat > "$SKILL_DIR/example.md" << EOF
# Example Usage

## Basic Invocation
/$SKILL_NAME

## With Arguments
/$SKILL_NAME arg1 arg2

## Expected Output
EOF

# Create reference.md
cat > "$SKILL_DIR/reference.md" << EOF
# Reference Documentation

## Frontmatter Fields

- \`name\`: $SKILL_NAME
- \`description\`: $DESCRIPTION
- \`disable-model-invocation\`: $DISABLE_MODEL_INVOCATION
- \`user-invocable\`: $USER_INVOCABLE
- \`allowed-tools\`: $ALLOWED_TOOLS
- \`context\`: $CONTEXT
- \`agent\`: $AGENT

## Argument Substitutions

- \`$ARGUMENTS\`: All arguments
- \`$ARGUMENTS[0]\`: First argument
- \`$0\`: First argument (shorthand)

## Dynamic Commands

Use \`!command\` syntax to run shell commands before skill execution.
EOF

echo "✅ Skill '$SKILL_NAME' created successfully at $SKILL_DIR"
echo "📁 Location: $SKILL_DIR"
echo "📄 Edit '$SKILL_DIR/SKILL.md' to add detailed instructions"