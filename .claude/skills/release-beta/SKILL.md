---
name: release-beta
description: Trigger the beta release GitHub Actions workflow
disable-model-invocation: true
allowed-tools: Bash(gh *) Bash(git *)
argument-hint: [optional-sha]
---

# Release Beta

Trigger the beta release workflow (`release-beta.yml`) via GitHub Actions.

## Steps

1. **Determine SHA**: If `$ARGUMENTS` is provided, use it as the commit SHA. Otherwise leave it blank (the workflow will use HEAD).

2. **Trigger the workflow**: Use `gh workflow run` to dispatch:
   ```bash
   gh workflow run release-beta.yml -f sha=<sha>
   ```
   Omit `-f sha=` if no SHA was specified.

3. **Monitor**: Show the user the link to the running workflow:
   ```bash
   gh run list --workflow=release-beta.yml --limit 1 --json url --jq '.[0].url'
   ```
