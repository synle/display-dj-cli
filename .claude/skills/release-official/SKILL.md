---
name: release-official
description: Trigger the official release GitHub Actions workflow
disable-model-invocation: true
allowed-tools: Bash(gh *) Bash(git *) Read
argument-hint: [optional-version]
---

# Release Official

Trigger the official release workflow (`release-official.yml`) via GitHub Actions.

## Steps

1. **Determine version**: If `$ARGUMENTS` is provided, use it as the version. Otherwise leave it blank (the workflow will read from Cargo.toml).

2. **Generate release notes**: Run `git log` to show changes since the last official published release:
   ```bash
   PREV_TAG=$(gh release list --exclude-drafts --exclude-pre-releases --limit 1 --json tagName --jq '.[0].tagName')
   git log "${PREV_TAG}..HEAD" --pretty=format:'- %h %s' --no-merges
   ```
   Show these to the user and ask them to provide optional release notes (or press enter to skip).

3. **Trigger the workflow**: Use `gh workflow run` to dispatch:
   ```bash
   gh workflow run release-official.yml -f version=<version> -f release_notes="<notes>"
   ```
   Omit `-f version=` if no version was specified (let the workflow use Cargo.toml).

4. **Monitor**: Show the user the link to the running workflow:
   ```bash
   gh run list --workflow=release-official.yml --limit 1 --json url --jq '.[0].url'
   ```
