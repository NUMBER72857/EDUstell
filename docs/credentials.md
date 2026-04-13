# Achievement Credentials

This slice adds first-party, non-transferable student achievement credentials to EduVault.

## Scope

Supported credential types:

- `scholarship_recipient`
- `fee_fully_funded`
- `academic_excellence`
- `attendance_recognition`

The design is intentionally narrow:

- credentials are off-chain records first
- credentials are non-transferable by design
- no token balances, no marketplace logic, no collectible metadata
- optional attestation anchor fields exist for a later on-chain or registry proof

## Data Model

Primary record: `achievement_credentials`

Key fields:

- `credential_ref`: stable public reference for later verification work
- `child_profile_id`: guardian-visible linkage
- `recipient_user_id`: optional student user linkage
- `school_id`: optional issuer context
- `achievement_type`
- `title`, `description`, `achievement_date`
- `issued_by_user_id`, `issued_by_role`
- `attestation_hash`, `attestation_method`
- `attestation_anchor`, `attestation_anchor_network`
- `metadata`

Privacy posture:

- full credential content stays in the application database
- only hash/reference/anchor data is intended for external attestation
- no sensitive student payload is required on-chain

## API

JSON endpoints:

- `POST /api/v1/credentials`
- `GET /api/v1/credentials`
- `GET /api/v1/credentials/{id}`

HTML pages:

- `GET /credentials`
- `GET /issuer/credentials`

Current auth limitation:

- the repo still uses bearer tokens without browser session auth
- the HTML pages therefore accept a pasted bearer token for local/internal use

## Authorization

Issue:

- `platform_admin`
- `school_admin` when bound to a verified `school_id`

View:

- `student` for credentials where `recipient_user_id == current user`
- `parent` for credentials attached to owned child profiles
- `school_admin` for credentials they issued
- `platform_admin`

## Lightweight Attestation Design

First version:

1. canonicalize credential issuance payload
2. hash it with SHA-256
3. store the hash off-chain with the credential record
4. optionally anchor the hash in:
   - a Stellar/Soroban event
   - a transaction memo / tx hash
   - a signed registry entry

That keeps the contract surface optional and small.

If a Soroban contract is added later, keep it minimal:

- method: `anchor_credential(bytes32 hash, string credential_ref)`
- emit event with hash + ref
- reject updates for the same `credential_ref`
- no ownership transfer methods
- no balances
- no token standards
