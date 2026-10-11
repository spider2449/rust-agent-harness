# Task 518 - v0.34.0 source-only publication

Authorized source: `7b972988f2f116b87b3770d66be1ef970e20b726`.

1. Reverify local/origin/live master, exact-head CI and tag/release absence.
2. Prepare Release Notes disclosing the explicitly approved OpenAI live waiver,
   unverified live behaviors and NSIS installer noncertification.
3. Create annotated v0.34.0 at the authorized source; push only that tag and
   verify local/remote annotation and peeled source identity.
4. Require successful tag push CI for that exact source before release creation.
5. Create the official nondraft, nonprerelease GitHub Release with zero uploaded
   assets; independently verify release, tag, master, CI and preserved reports.

Stop on any failed prerequisite; never substitute another SHA or repair an
existing tag. No production, version, dependency or release-source commit change.
This local plan remains outside the immutable release source. Publication
evidence and Release Notes are retained under target/task518.
