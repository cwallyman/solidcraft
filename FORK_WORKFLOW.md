# Browser development workflow

This fork is used to develop and test SolidCraft browser features, especially touch and Apple Pencil input, and to contribute changes to tangency-labs/solidcraft. Development of the native iPad app is paused.

Both Codex and Claude must follow the upstream AGENTS.md and the relevant skills it names. The upstream CLAUDE.md points to AGENTS.md. This document is fork-specific and belongs only on browser-test; do not include it or the browser-test Pages workflow in upstream pull requests.

## Start a feature

1. Fetch the latest main branch from https://github.com/tangency-labs/solidcraft.git. Create a feature branch from that upstream commit, not from browser-test or the fork's iPad default branch. Check upstream's current contribution rules and open issues and PRs before starting.
2. Give Codex and Claude separate branches and separate checkouts or git worktrees when they work at the same time. Avoid making concurrent edits in the same working tree. Each agent should review the current branch and working tree before editing.
3. Keep each feature branch focused on one upstream change. Follow the upstream rules for tests, evidence, draft PRs and CI. Never force-push a shared branch or discard another agent's changes.

## Test on GitHub Pages

The browser-test branch publishes the browser app to https://cwallyman.github.io/solidcraft/ when its Pages workflow succeeds. It is a shared integration branch: the site contains all changes currently integrated there, and only one build is published at a time. The site is public and is not suitable for private data.

Cherry-pick the feature's relevant commits into browser-test, resolve conflicts carefully, and push browser-test to trigger the build. Inspect the workflow run and confirm deployment before reporting a test as available. The first build may take a long time. Test touch, Pencil and browser behavior on the intended device. GitHub Pages cannot validate the native iPad shell or native Files integration.

Keep the original feature branch clean and open the upstream PR from that branch, never from browser-test. Do not cherry-pick the testing branch's workflow or this file into upstream PRs. Record which feature commits are included in browser-test so they can be refreshed or removed safely. Coordinate before changing that shared branch. After upstream merges a feature, fetch upstream again before starting the next one.

## Starting a session

Tell the agent: "Read FORK_WORKFLOW.md on cwallyman/solidcraft's browser-test branch, then follow upstream AGENTS.md and its relevant skills. Create a fresh feature branch from current upstream main." Agents working on a fresh upstream-based feature branch can read this file from GitHub or a separate browser-test worktree; they should not copy it into the feature branch.
