# CI Coverage

PFTerminal CI currently runs only checks backed by available GitHub-hosted Linux
x64 runners. This is intentional: prior Mac, Windows, ARM, and self-hosted
runner jobs were failing before checkout with no executed steps, which created
red status noise without testing the code.

## Active Push/PR Coverage

- Formatting, spelling, manifest, and dependency checks.
- Rust cargo checks on Linux x64.
- Rust nextest on Linux x64.
- Bazel test, clippy, and release-build verification on Linux x64.
- SDK checks on hosted Linux x64.
- V8 canary coverage on hosted Linux x64 only.

## Disabled Until Runners Exist

- macOS Rust and Bazel checks.
- Windows Rust and Bazel checks.
- Linux ARM64 Rust, Bazel, and V8 checks.
- Self-hosted runner jobs using labels such as `PFTerminal-linux-x64`,
  `PFTerminal-linux-arm64`, `PFTerminal-windows-x64`, or
  `PFTerminal-windows-arm64`.

These disabled jobs were not real failing tests. They failed before checkout
because GitHub could not assign a runner.

## Restoration Requirements

Before re-enabling platform checks:

1. Confirm the runner label exists and is attached to this repository or org.
2. Confirm a trivial workflow using that exact `runs-on` value reaches checkout.
3. Re-enable one platform leg at a time.
4. Make the platform check required only after it has passed on at least one
   fresh push.

Until then, CI should stay green and honest rather than displaying checks that
never run.
