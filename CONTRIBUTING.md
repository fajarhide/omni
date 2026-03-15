# Contributing to OMNI

We love your contributions! OMNI is built on a mission to make context usage as efficient as possible. Here’s how you can help:

## Getting Started

1. **Fork the repo** and clone it locally.
2. **Install dependencies**:
   - Zig 0.15.2 (Critical for the core engine)
   - Node.js 18+ (For the MCP gateway)
3. **Explore the codebase**:
   - `core/src/filters/`: Join the mission by adding specialized semantic filters.
   - `src/`: Refine the MCP server or caching logic.

## Contribution Workflow

1. **Bug Reports & Feature Requests**: Open an issue describing the context and the problem/idea.
2. **Pull Requests**:
   - Create a fresh branch from `main`.
   - Ensure your code follows `zig fmt` and `npm run lint` (if applicable).
   - Write tests for new filters and run `zig build test`.
   - Update `CHANGELOG.md` with your changes.

## Development Core

Before modifying the core, please read:
- [DEVELOPMENT.md](docs/DEVELOPMENT.md)

## Code of Conduct

Be kind, respect the semantic integrity of the project, and help us build the most efficient engine for the AI era.

Thank you for contributing! 🚀
