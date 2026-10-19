# GLib 0.18.5 security backport

This directory contains the published `glib 0.18.5` crate under its original MIT license. It is a required build dependency, not a generated artifact. The GTK3 chain cannot switch to GLib 0.20 without incompatible API changes.

The semantic change is limited to two lines of `src/variant_iter.rs` that differ from the published source: the `g_variant_get_child` out-pointer is mutable and is passed as `&mut p`. This is the upstream fix from [gtk-rs PR #1343](https://github.com/gtk-rs/gtk-rs-core/pull/1343), backported for [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html). Other library behavior is unchanged. `src/lib.rs` also allows two newer compiler style lints (`unused_parens`, `mismatched_lifetime_syntaxes`) to retain the upstream formatting without unrelated edits. These are not advisory exclusions.

Cargo selects this source through `[patch.crates-io]`; no advisory-ignore entry is added. A local source is not a newly patched crates.io release: retain this provenance when interpreting dependency scanner results. The application runs regression tests for forward/backward string iteration in optimized builds. Remove this backport once the GTK dependency chain can use an upstream fixed release.

The original crate checksum in crates.io is `233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`. The upstream source, tests, copyright and license are retained. This backport fixes this specific advisory, not every potential defect in GLib/GTK/WebKit.
