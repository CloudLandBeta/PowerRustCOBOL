<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — The Authentication control and the identity-provider backend

- **Status:** draft (needs `/clarify`; the flow is chosen per provider, 2026-10-19, but each provider's capabilities still have to be verified)
- **Folder:** specs/WASM-009-authentication-control-and-identity-providers/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** operator item **11** — credentials are obtained by a control made for authentication, able to deal with **reCAPTCHA v3**, **account-creation requests**, **password recovery** and optional **MFA**;
  every Web/mobile project gets a backend that handles authentication, reset and creation of users against **AWS Cognito**, **Keycloak** or **Microsoft Entra**. It overlaps with item 3 (the bearer JWT), which
  WASM-003 owns on the validating side; this spec owns the *obtaining* side.
- **Depends on:** WASM-003 (the backend validates the token), WASM-005 (the calls), WASM-007 (its look). **Related:** the existing `AwsCognito` control.

## 1. Overview

A browser application needs a way for the user to sign in, create an account, recover a password and, where required, prove a second factor — and it needs to do so **without the browser ever holding a secret** and with
protection against automated abuse. The **Authentication control** is the visual part: it shows the screens and raises events. The **identity backend** is the part every Web/Mobile project receives (WASM-003 R22):
backend programs that talk to the chosen identity provider and return a token the frontend then sends with every call (WASM-005 R8). The developer picks a provider in the project's settings; they never touch a token.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **No identity code.** No JWT, OIDC, Keycloak, Microsoft Entra or reCAPTCHA code anywhere in the workspace; the only OIDC mention is a comment in the dependency guard
  (`cobolt-compiler/tests/dependency_guards_063.rs:73`).
- **`AwsCognito` control** (one of the 16 AWS controls): `SignUp`, `Confirm`, `SignIn`, `GetAttribute` and `SignOut`, routed through the AWS MCP server (`cobolt-runtime/src/aws/routes.toml:758`,
  `interpreter/aws.rs`); tokens stay inside the runtime (`cobolt-compiler/src/lib.rs:7266`); it needs the `aws` feature and the developer's AWS credentials. It is a *desktop/backend* control; it cannot run in a
  browser (WASM-004 class C).
- **Client-side auth that exists:** RestClient's `AuthType` None | Bearer | Basic | APIKey (`cobolt-forms/src/connections.rs:78`) — it sends a token, it does not obtain one.
- **A Bearer-checking server exists only for MCP** (`cobolt-project-tools/src/http.rs:224-229`).

## 2. Goals / Non-goals

- **Goals:**
  - An **Authentication** control with sign-in, sign-up, confirmation, forgot/reset password and optional MFA screens, and a sign-out action.
  - **reCAPTCHA v3** protection of the sensitive actions, verified on the backend.
  - A provider-neutral identity backend with adapters for **AWS Cognito**, **Keycloak** and **Microsoft Entra**.
  - Tokens handled so that script on the page cannot steal the long-lived credential; the developer writes no token code.
  - Events and properties the form can use (who is signed in, when they sign out).
- **Non-goals:**
  - Social login and federation (Google, Apple, …), passkeys/WebAuthn, an authorisation/permissions model and an administration console — each is listed as a proposed spec.
  - Replacing the `AwsCognito` control, which stays for backend-side administration.
  - Hosting an identity provider.

## 3. User stories

- As a developer, I want to drop an Authentication control on a form and choose "Cognito", so that users can sign up and sign in without my writing any of it.
- As a user, I want to recover a forgotten password and to confirm my e-mail, as on any modern site.
- As a developer, I want a bot-protected sign-up that does not show my users a puzzle, so that abuse is low and conversion is high.
- As a security reviewer, I want to know that the browser never sees a client secret and that a stolen page script cannot walk away with a long-lived credential.

## 4. Requirements (EARS)

**The control**
- **R1 (ubiquitous):** An **Authentication** control shall exist in the toolbox of web and mobile projects. It shall provide the screens **Sign in**, **Create account**, **Confirm account** (e-mail or phone
  code), **Forgot password** (request, then code and new password), an optional **MFA challenge** and a **Sign out** action; the developer chooses which are present, and their text is translatable.
- **R2 (ubiquitous):** The control shall look and be styled like any control (properties, `CssClass`, `Style`, the form stylesheet, WASM-007), shall set `autocomplete` correctly (`username`, `current-password`,
  `new-password`, `one-time-code`) so that password managers work, and shall be keyboard- and screen-reader-operable.
- **R3 (ubiquitous):** The control shall expose to COBOL: events (**onSignedIn**, **onSignedOut**, **onSignUpCompleted**, **onPasswordResetRequested**, **onMfaRequired**, **onError**), properties (**State**,
  the signed-in **UserName** and **Email**, **Mode**) and methods (**SignOut**, **Refresh**, **GetClaim**), named at `/plan` and registered in the closed vocabulary and the System KB.

**Abuse protection**
- **R4 (optional):** Where reCAPTCHA v3 is enabled, the control shall obtain a token for each protected action (sign-in, sign-up, forgot-password) without showing a challenge, send it with the request, and the backend
  shall verify it with the provider using a secret that exists only on the backend, check the action name and the hostname, and refuse a score below a configurable threshold (default 0.5) with the same generic
  message used for a wrong password. The site key is public configuration; the secret key is never in the frontend.

**Tokens**
- **R5 (ubiquitous):** The **access token** shall be held in memory only (never in `localStorage` or `sessionStorage`); the **refresh credential** shall not be readable by page script — an `HttpOnly`, `Secure`,
  `SameSite` cookie set by the backend on the web, the operating system's secure store on mobile — and the runtime shall refresh silently before expiry. When refresh fails the control shall raise **onSignedOut**
  and the application shall be able to show the sign-in screen.
- **R6 (ubiquitous):** Tokens shall never be written to a log, a URL or the page's visible text, and the developer shall have no need to read one (`GetClaim` returns a claim's value, not a token).

**The identity backend**
- **R7 (ubiquitous):** The identity backend shall be a set of generated **public** programs of the project's Backend (WASM-003 R9, R22): sign-in, sign-up, confirm, forgot-password, reset-password, MFA
  challenge/verify, refresh and sign-out, calling the configured provider and returning tokens only as R5 allows.
- **R8 (ubiquitous):** Provider support shall sit behind one **adapter interface**, so that a fourth provider is an adapter and not a change to the control. The three adapters shall support, within what each provider
  offers, authentication, registration, confirmation, password recovery, an MFA challenge, refresh, sign-out/revocation and the basic attributes; a table in the plan shall list per provider what is not supported and
  what the control does instead.
- **R8b (ubiquitous):** The sign-in **flow is chosen per provider** (operator, 2026-10-19): each adapter shall declare whether it uses the **in-app flow** — the control's own screens, the backend exchanging the credentials with the provider — or the provider's **hosted flow** — the
  browser or web view goes to the provider's sign-in page, brokered by the backend with an authorisation code and PKCE — and the control shall present the matching screens. Where the flow is hosted, sign-up, recovery and MFA are the provider's pages, and the control shall still raise the same
  events (R3), so a form's handlers do not depend on the flow. The capability table (R8) shall list the flow of each provider.
- **R9 (ubiquitous):** Provider settings (issuer or region/realm/tenant, client id, any client secret, the reCAPTCHA secret) shall be set in the Backend's configuration through an IDE settings page with a **Test
  connection** action, and shall never be written to a form, a copybook or any frontend file.

**Abuse and enumeration**
- **R10 (ubiquitous):** Failure messages shall not reveal whether an account exists (the same response for an unknown user and a wrong password, and for "reset requested" whether or not the address is known); the backend
  shall rate-limit sign-in, sign-up and reset requests per source and per account and rely on the provider's lockout; passwords shall never be logged.
- **R11 (state):** While an application requires sign-in for a form (Q4), the application shall show the Authentication form to a signed-out user and the requested form after sign-in.

**Assurance**
- **R12 (ubiquitous):** Automated tests shall run the full flows against a **local identity provider** (a Keycloak instance in a container and a mock for the others); tests against live Cognito and Entra tenants belong to the
  operator, as the live AWS check of spec 103 did, and the acceptance records which ones ran.

## 5. Acceptance criteria

- [ ] **AC1** — The control shows each screen; the developer's choice of screens is honoured; strings translate; the axe-style accessibility check reports no critical issue.
- [ ] **AC2** — Against the local Keycloak: create account, confirm, sign in, call an exported program with the resulting token (accepted), sign out; forgot-password and reset; MFA challenge when the realm requires it.
- [ ] **AC3** — A wrong password and an unknown user produce the same message and similar timing; a reset request for an unknown address looks like one for a known address.
- [ ] **AC4** — With reCAPTCHA enabled, a request with no token, a replayed token, a token for another action or hostname and a score below the threshold are each refused; a good one is accepted (the provider is mocked).
- [ ] **AC5** — After sign-in, `localStorage`/`sessionStorage` contain no token (a test reads them); the refresh credential is not visible to `document.cookie`; a forced expiry triggers one silent refresh and the call
  proceeds.
- [ ] **AC6** — A scan of the frontend bundle, the forms and the copybooks finds no client secret, reCAPTCHA secret or signing key; the same values in the backend configuration are accepted.
- [ ] **AC7** — Token strings do not appear in the backend's logs or the browser console for a full flow (a test greps both).
- [ ] **AC8** — Each adapter passes the same adapter-conformance suite against its mock; the capability table lists what each does not support.
- [ ] **AC9** — The control round-trips through `.cfrm`; `ControlType::ALL`-driven tests pass; the System KB freshness test passes.

## 6. Constraints & steering check

- **The frontend↔backend channel (WASM-INDEX D21, operator 2026-10-24):** whatever this spec puts on the channel between the frontend and the backend obeys the series-wide ruling — the channel is asymmetrically encrypted with a key pair generated when the application starts; a broken connection is retried with a timeout of 10 s, raised by 10 s on each retry, for at most 3 cycles, then an exception is raised; while the frontend retries, all mouse and keyboard input is blocked and the "connecting" modal of WASM-016 R20 (WASM-012 R15 on a device) is shown, appearing when the connection breaks and disappearing when it is re-established. The ruling is owned by WASM-003 R24–R26, WASM-016 R20–R21, WASM-012 R15, WASM-005 R18, WASM-014 R18 and WASM-013 R17; this spec adds nothing of its own to it and must not contradict it.
- **i18n:** yes — the control's default texts and the settings page, six languages. The text the end user of the developer's application sees is translatable by the developer.
- **Generated-code contract:** the identity programs are generated artefacts (banner, regenerate on Build) and never hand-edited; COBOL they contain follows the extensions ruling and the call-comment ruling.
- **System KB:** **required** — a new control, its properties, events and methods; regenerate `chunked.data`.
- **Credentials:** the project's no-credentials-in-commits rule applies to the test fixtures and the example project; the fact that a form has properties where a key can be typed (and that the project's notes record live keys having been found in demo forms before) is why R6, R9 and AC6 exist.
- **Docs:** `docs/` untouched on 2.x.x.
- **Fix vs feature:** **feature**.

## 7. Open questions

- **Q1 — The design risk: a password typed into *our* form — decided: the flow is chosen per provider** (operator, 2026-10-19), option (c) below; R8b states it. What remains for `/clarify`: verify each product's current capability (the paragraph below is general
  knowledge) and fix the default flow of each adapter — recommended: in-app screens for Cognito, the hosted flow for Entra and Keycloak unless the tenant is configured for the in-app one. The original analysis follows.
  **The risk:** The operator's model — the application shows its own sign-in form and the backend exchanges the password with the provider — is the *password grant*. For
  **Microsoft Entra** that flow is not supported for accounts that require MFA or are federated, and Microsoft steers new work to a browser-redirect flow; for **Keycloak** the password ("direct access") grant is
  disabled by default and discouraged in current OAuth guidance; **AWS Cognito** supports password and SRP authentication with MFA challenges natively. (Stated from general knowledge of the three products —
  **verify each at `/clarify`**, including whether Entra External ID's *native authentication* and Keycloak's own flows fit.) Options: (a) keep the in-app form where the provider allows it and say honestly, in the
  capability table, that Entra and Keycloak need their tenant configured for it; (b) have the Authentication control launch the provider's **hosted** sign-in (authorization code with PKCE, brokered by the backend) and
  use the in-app screens only for Cognito; (c) support both, chosen per provider. Recommended: (c). It changes R1, R7 and R8 materially, so it must be settled first.
- **Q2 — Backend-issued token or the provider's.** Recommended for 2.0: the provider's access token, validated by the backend against the provider's keys (WASM-003 R10) — no token minting, no key custody. A
  backend-issued session token is a later option.
  **Decided (operator, 2026-10-22):** the recommendation: the provider's access token, validated by the backend.
- **Q3 — Refresh cookie needs HTTP,** while the series' contract is a WebSocket (WASM-003 Q3). Confirm that the backend also serves a small HTTPS surface for the cookie and the provider's redirects.
  **Decided (operator, 2026-10-22):** confirmed.
- **Q4 — Route protection:** a form marked "requires sign-in", a property of the application shell (spec 049), or a handler's explicit check? Recommended: a form-level property.
  **Decided (operator, 2026-10-22):** a form-level property.
- **Q5 — MFA kinds in 2.0:** TOTP only, or also SMS/e-mail codes? It follows what each provider offers; list them in R8's table.
  **Decided (operator, 2026-10-22):** TOTP, and SMS and e-mail codes as well.
- **Q6 — reCAPTCHA v3 and privacy:** it loads a third party's script and sends data to it; the consent text and the regional rules (LGPD, GDPR) are the developer's to satisfy; should the control provide a consent hook?
  Alternatives (hCaptcha, Cloudflare Turnstile) are a later adapter.
  **Decided (operator, 2026-10-22):** the control offers a consent hook.
- **Q7 — Mobile:** the OS secure store through a Tauri plugin and biometric unlock are assumed; confirm with the mobile-platform spec proposed in WASM-INDEX. Note that iOS requires "Sign in with Apple" when other social
  logins are offered — relevant only once federation is added.
  **Decided (operator, 2026-10-22):** confirmed.
