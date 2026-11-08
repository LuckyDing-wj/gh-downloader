# Release workflow rules

- Keep `.github/workflows/build.yml` triggered for pull requests, pushes to every branch, version tags (`v*`), and manual dispatch. Every code change should run validation and build platform installers.
- GitHub Release publication is tag-only: create/push a `v<version>` tag matching both `package.json` and `src-tauri/tauri.conf.json` versions. Branch and PR runs produce downloadable Actions artifacts, not Releases.
- Preserve `if-no-files-found: error` and the release asset-empty check so packaging failures fail the workflow visibly.
- When changing release triggers, version checks, bundle targets, or artifact paths, verify branch/PR runs build artifacts and a matching version tag runs the publish job.
