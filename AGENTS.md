# Agent notes for this fork

See `FORK.md` for the fork's features and the build scripts in `scripts/fork/`.

## Deploy after every fix (standing instruction)

After a fix or feature is committed and its tests pass, always rebuild and redeploy the
live `stump-fork` container without asking:

1. Build on l5: `l5run -- 'bash scripts/fork/build-local.sh stump:fork-<shortrev>'`
   (g3 and l5 share the same absolute path; l5 has no host Rust toolchain, the scripts use containers).
2. Bring the image to g3: `ssh l5 'docker save stump:fork-<rev> | gzip -1' | gunzip | docker load`.
3. In `/mnt/data2/containers/stump-fork/`: back up `docker-compose.yml`
   (`docker-compose.yml.bak-<date>-<topic>`), set the image tag where the compose file keeps it and add a comment
   line (branch/rev, date, what changed, migration or not, rollback image):
   - when the file has the line `x-stump-image: &stump_image ${STUMP_FORK_IMAGE:-stump:fork-<rev>}` (it does once the
     isolated checker for friends' uploads is installed), change the tag **there**: that one line pins both
     `stump-fork` and `stump-intake-probe`, and the two must always run the same build;
   - otherwise set `image: ${STUMP_FORK_IMAGE:-stump:fork-<rev>}` on `stump-fork`.
4. If the change has a migration, first snapshot the db to `config.backup-<UTC ts>-db`.
5. `docker compose config -q && docker compose up -d --no-build stump-fork`, then check
   `docker ps` and `docker logs stump-fork` for a clean start. When the compose file has a `stump-intake-probe`
   service, recreate **both**: `docker compose up -d --no-build stump-fork stump-intake-probe` (with another build the
   checker counts as out of date and friends' uploads close), and check `docker logs --since 2m stump-intake-probe`:
   every `selftest <check>: ok` line (no `WARN` or `FAIL`), then `stump-intake-probe <rev> ready`.

Do not push to a remote without asking.
