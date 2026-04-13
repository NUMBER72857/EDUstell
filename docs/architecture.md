# Architecture

EDUstell starts as a modular monolith. The API layer handles HTTP concerns only. Application orchestrates use cases. Domain holds business rules. Infrastructure provides SQLx and external adapters. Blockchain integration remains isolated behind its own crate.

The first implemented vertical slice is auth and RBAC:

- `domain::auth` defines roles, users, claims, and public user views
- `application::auth` defines register/login/refresh/logout/current-user use cases
- `application::ports` defines auth repository, hashing, and token interfaces
- `infrastructure::auth` implements SQLx-backed auth persistence, Argon2 hashing, and JWT issuance/verification
- `apps/api` exposes auth endpoints and role enforcement at the HTTP boundary
