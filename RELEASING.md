# Release procedure

The first 0.1.0 candidate is prepared for review, not published. No publishing
credentials or automatic upload workflow are configured. The `rustpfm` Cargo
package name is provisional until successful crates.io registration; a GitHub
repository name does not reserve a crate name.

1. Review and merge the release PR after all Quality gates pass. Review API
   semantics, compatibility policy, benchmark evidence, and crate contents.
2. When publication is explicitly approved, replace the candidate changelog
   label with the publication date, enable the intended registry in Cargo.toml
   (`publish = ["crates-io"]`), and update README installation/status wording.
   Use a reviewed branch and the complete quality checks for these final edits.
3. Run `./scripts/check.sh`, both MSRV feature configurations, and live Netpbm
   checks on the final main commit. Run `cargo publish --dry-run --locked` with
   the approved package name, inspecting the source archive and its metadata.
4. Tag that checked commit as `v0.1.0`, create release notes, then publish with
   `cargo publish --locked` using the maintainer's crates.io credentials or a
   separately reviewed trusted-publishing setup. Do not place credentials in
   repository files or chat.
5. Verify a clean downstream install from crates.io, both default and ndarray
   configurations, documentation links, and the published version. Attach the
   checked crate archive and checksum to the GitHub release.

Publishing versions is irreversible in practice; do not publish an unfinished
scaffold or silently overwrite version identity. Subsequent releases update
Cargo.toml, Cargo.lock, changelog, documentation and benchmark provenance as
needed. A version bump alone never relabels a historical measurement.
