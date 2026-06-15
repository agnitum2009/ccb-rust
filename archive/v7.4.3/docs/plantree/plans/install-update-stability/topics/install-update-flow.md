# Install Update Flow

Date: 2026-06-04

## Fresh Install

Fresh install is owned by `install.sh install`.

Required flow:

1. Detect language with `CCB_LANG`, `LANG`, `LC_ALL`, or `LC_MESSAGES`.
2. Confirm root/sudo profile if effective uid is root.
3. Check WSL compatibility and backend environment.
4. Select Python 3.10+.
5. Install required or optional Python packages only when missing.
6. Check terminal backend requirements.
7. Prepare install tree:
   - source/dev mode uses a live source root
   - release mode copies release content to the install prefix
8. Create managed venv when policy says to use one.
9. Write install metadata for release installs.
10. Install wrappers and bin links.
11. Run real installed entrypoint smoke checks.
12. Install inherited skills, settings, tmux helpers, and other static assets.
13. Provision optional Role Packs and tools.
14. Print install identity and next actions.

Core install success stops at step 12. Role Pack and tool provisioning are
post-install checks unless the user explicitly forces them as required.

## Managed Update

Managed update is owned by Python `ccb update`.

Required flow:

1. Resolve supported platform and target version.
2. Download the matching release artifact.
3. Extract to a temporary staging root.
4. Run the staged release `install.sh install` with optional provisioning
   disabled:
   - `CCB_INSTALL_ROLES=0`
   - `CCB_INSTALL_NEOVIM=0`
5. Verify the newly installed entrypoint and read new build metadata.
6. Print update outcome.
7. Run post-update provisioning through the newly installed `ccb`, not through
   the old updater process.
8. Report optional provisioning warnings without making the core update look
   failed unless the user forced required provisioning.

The key boundary is step 7. Once release files are installed, Role Pack and
tool semantics belong to the new release. The old updater process should only
bootstrap the new post-update runner.

## Post-Update Runner

The post-update runner should be a CLI entrypoint in the newly installed CCB,
for example an internal command such as:

```text
ccb __post-update --from-version <old> --to-version <new>
```

Responsibilities:

- Refresh or locate the `agent-roles-spec` catalog.
- Canonicalize legacy Role Pack ids before status comparison.
- Refresh installed Role Packs only when source version or digest changed.
- Skip Role Pack updates when status is already `current`.
- Install newly available Role Packs only after interactive confirmation.
- Provision Neovim only when requested or accepted.
- Emit bilingual summary and remediation messages.

## Failure Classification

Failures must be separated:

- blocking core update failure:
  download failed, extraction unsafe, installer failed, installed entrypoint
  smoke check failed
- non-blocking optional provisioning warning:
  catalog unavailable, Role Pack tool install failed in optional mode, Neovim
  unavailable in optional mode, Droid registration failed
- required provisioning failure:
  user set a force/required env var and the dependency failed

User output should make this distinction explicit. A successful core update
with optional Role Pack warning should say the update completed and then show
the optional warning plus retry command.

## Current Known Drift

The v7.2.9 incident showed that old updater code can continue after installing
new files and try to update a legacy `ccb.archi` source that no longer exists.
Moving post-update provisioning into the new installed entrypoint prevents this
class of old-code/new-layout mismatch.
