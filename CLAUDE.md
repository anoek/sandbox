# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

## Project Overview

Sandbox is a Linux containerization tool that creates lightweight copy-on-write
views of the system for safely running semi-trusted programs. It uses OverlayFS
to track file changes and allows selective acceptance of modifications.

## Development Commands

### Building

-   `make build` - Build debug binary
-   `make build-release` - Build release binaries for multiple architectures
-   `make build-coverage-binary` - Build with coverage instrumentation
-   `make watch` - Watch for changes and rebuild automatically

### Testing

-   `make test` - Run all tests with coverage (requires sudo)
-   `TEST=test_name make quick-test` - Run specific test(s)
-   `make full-test` - Run comprehensive VM-based tests (requires VM setup)
-   `make coverage-report` - Generate and view coverage report at coverage/html/index.html

### Code Quality

-   `make lint` - Run Rust linter
-   `make clippy-fix` - Auto-fix linting issues
-   `cargo fmt` - Format code
-   `make audit` - Security audit of dependencies
-   `make ready-for-commit-tests` - Run all pre-commit checks (formatting, linting, spelling)
-   `make install-pre-commit-hooks` - Install git pre-commit hooks

### Installation

-   `make install` - Install sandbox locally
-   `make uninstall` - Remove installed sandbox

## Architecture

### Core Components

-   **src/main.rs** - Entry point and CLI handling
-   **src/actions/** - Implementation of sandbox actions (accept, reject, status, etc.)
-   **src/config/** - Configuration parsing and management (CLI args, env vars, config files)
-   **src/container/** - Core containerization logic using Linux namespaces
-   **src/filesystem/** - OverlayFS management and file operations
-   **src/utils/** - Utility functions and helpers

### Key Concepts

1. **Sandboxes** - Named containers with isolated filesystems
2. **OverlayFS** - Used to create copy-on-write views of the host filesystem
3. **Namespaces** - Linux namespaces for process and network isolation
4. **Storage Directory** - Default ~/.sandboxes/ stores sandbox data

### Testing Strategy

-   Integration tests in `tests/` directory
-   Test fixtures in `tests/fixtures/`
-   100% line coverage requirement for PRs VM-based testing infrastructure in `vm-testing/` for cross-platform validation ## Important Implementation Notes

1. **Privilege Handling**: The binary requires setuid root for namespace operations. The Makefile handles this during build.

2. **Config Loading Order** (most to least specific):

    - CLI arguments
    - Environment variables (SANDBOX\_\*)
    - .sandbox.conf/.sandbox.toml in current and parent directories
    - ~/.config/sandbox/config.(conf|toml)
    - /etc/sandbox.(conf|toml)

3. **File Operations**: When working with file accept/reject operations, be aware of OverlayFS limitations regarding concurrent modifications to upper/lower filesystems.

4. **Coverage Requirements**: All PRs must maintain very high test coverage. Use `make coverage-check` to verify.

5. **Cross-Platform**: Supports x86_64 and aarch64 architectures with both GNU and musl libc variants.

6. **New tests**: New tests should be added to the `tests/` directory with a filename prefixed with `ai_test_`. Most tests should use the default name provided by the SandboxManager, if you provide a name, it needs to be prefixed with `sandbox-coverage`.
