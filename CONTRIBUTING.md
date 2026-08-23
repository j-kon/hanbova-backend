# Contributing to Hanbova

Thank you for contributing to Hanbova! We welcome open-source contributions from developers across Africa and worldwide.

---

## 1. Code of Conduct
By participating in this project, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

---

## 2. Development Workflow

1. **Fork and clone** the repository.
2. Create a new branch: `git checkout -b feature/my-feature`
3. Run setup: `make setup`
4. Make your changes adhering to code style and architecture.
5. Verify linting and tests:
   ```bash
   make test
   make lint
   make format
   ```
6. Commit with clear, descriptive commit messages:
   ```bash
   git commit -m "feat(protected-payments): implement timelock validation"
   ```
7. Open a Pull Request on GitHub.

---

## 3. Engineering Guidelines

* **Pure Domain Independence**: Do not couple `crates/hanbova-core` to HTTP frameworks, database libraries, or specific wallet SDKs.
* **Security Defaults**: Never commit credentials, seed phrases, or private keys. Always use structured errors and input validation.
* **Testing**: All new features, payment transitions, and models must include unit tests.
* **No Speculative Dependencies**: Only introduce new dependencies if there is a strict technical necessity.
