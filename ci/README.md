# `ci/` — the workflow, and why it is not installed

`ci/ci.yml` is a GitHub Actions workflow that belongs at `.github/workflows/ci.yml` and is not
there. This directory is a holding place, not a design: a workflow parked outside
`.github/workflows/` does not run, is not scheduled, and is not consulted by anything. It is a
file waiting for somebody with the right credential.

**Until it is installed, this repository has no CI.** Nothing runs the test suite when a commit
lands, nothing runs it on a pull request, and no tick anywhere reflects the state of this code.
Every gate figure quoted in `docs/STATUS.md`, `testdata/README.md`, `web/README.md` and the
milestone documents — every *green*, every T-state count, every `67/67`, every mutation that
went red — was produced by a person typing a command on their own machine and writing down what
came back. That is a real form of verification and this project takes it seriously; it is not
continuous integration, and the two must not be allowed to read as the same thing. A reader who
sees a repository with a `ci/` directory and infers that changes are gated somewhere has
inferred wrongly, which is why this paragraph is the second one on the page rather than a
footnote.

## Why it is here instead of `.github/workflows/`

The cause is dull and it is worth stating exactly, because the interesting explanations are all
wrong. It is not a design decision, not a policy about vendoring, and not an unfinished
migration. **The GitHub credential available to the sessions that have worked on this repository
does not carry the `workflow` OAuth scope**, and GitHub refuses any push whose diff touches
`.github/workflows/**` from a token without it. The refusal is on the *path*, not on the branch,
the content or the author: every adjacent path pushes without complaint, and that one does not.
So the file was written, reviewed and committed to a branch, and the branch could never be
pushed — which left the workflow's only copy on a stale local branch, drifting further from
`main` with every merge, until it was moved here.

That history is the reason this file is worth reading rather than deleting. A workflow that
cannot be pushed is indistinguishable, from the outside, from a workflow nobody wrote.

## Installing it

One command, from the repository root:

```sh
cp ci/ci.yml .github/workflows/ci.yml
```

Then commit and push it with a credential that carries the **`workflow`** scope. A personal
access token needs that scope ticked; a GitHub CLI login obtains it with
`gh auth refresh -h github.com -s workflow`; pushing over SSH with your own account's key avoids
the question, because the restriction is a property of the OAuth token rather than of the
transport. If the push is rejected with a message naming `workflow`, the scope is what is
missing — the file is fine.

Keep this copy after installing it, or delete it, but do not keep both and edit only one. Two
copies of a workflow is the same defect this project catalogues at length in `docs/STATUS.md`:
one fact in two files, and the correction landing only in the one being read.

## What it does, in the order it will matter to you

The first job runs `sh web/gate.sh` — the project's own gate, unchanged and not restated. That
script defines what green means here, a developer runs it locally by that name, and a workflow
that listed its steps again in YAML would be a second definition free to drift from the first.
Around it the job does the two things the script cannot do for itself: it installs the ALSA
development headers, without which `crates/page`'s audio dependency refuses to build at all on a
bare Linux runner, and it fetches the corpora. **The fetch is not optional and cannot be skipped
by configuration.** `crates/testsupport` makes a missing corpus a failure, and refuses the
opt-out whenever `CI` is set — so under a pipeline the choice is between fetching the data and
going red, which is the whole point of that policy.

The second job runs the three gates that carry `#[ignore]`: `zexdoc_conformance`,
`zexall_conformance`, and `the_memptr_exerciser_reports_the_verdict_this_core_earns`. All three
now close their ignore text the same way — *"Nothing runs that automatically: this repository has
no CI, and `ci/README.md` says why"* — which is accurate, and which points here.

*This sentence used to report that all three said **"CI runs it that way"**. That was a claim
about a pipeline that does not exist, made in the one file whose entire subject is that it does
not exist; the ignore strings were corrected and this line was not, which is the only reason the
irony is still legible.* The deference now runs one way only, and that puts the next staleness on
the other foot: **installing the workflow falsifies all three strings at once.** Whoever copies
`ci.yml` into `.github/workflows/` rewrites them in the same commit, or leaves three gates
telling the reader there is no CI while a CI runs them.

`docs/STATUS.md` names that exact shape as the project's headline defect — an `#[ignore]`d gate
that no pipeline executes is not a gate — so this job is the difference between three oracles and
three test functions that are never called.

The third job exists because of something that was measured rather than feared. With
`testdata/fuse` moved aside, the suite once exited 0 with 87 passing tests, byte for byte
indistinguishable from a full green run: five tests verified nothing, the count did not change,
and the skip notice went unseen because libtest captures stdout for tests that pass. The guard
that prevents that is on by default now, and this job proves it is still on — that an undeclared
absence fails, that the opt-out is refused under `CI`, and that both pre-rename spellings of the
variable are hard errors rather than no-ops. It asserts the failure *message* and not merely a
non-zero exit status, because a typo in a package name and an armed guard both exit non-zero, and
a check that reads only the exit code would call the typo a success.

## What a green tick will and will not mean

It will mean the workspace compiles on a second machine, that `cargo fmt` and both `clippy` runs
are clean, that the whole suite passed against corpora that were actually present, that the
browser build links and its module imports and exports are the ones the page provides and calls,
and that the two instruction exercisers and the MEMPTR oracle returned the verdicts this core
earns.

It will not mean the page renders, that a keypress arrives, that a `Ctrl` chord can be cancelled,
or that the thing is playable. `web/gate.sh` says so in its own verdict block and this workflow
does not improve on it: those are properties of a browser, a GPU, a keyboard and a person, and
`web/README.md` is where such runs are recorded. A build gate that is mistaken for a playability
gate is worse than no gate, because it stops people looking.
