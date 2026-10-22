<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — the Authentication control and the identity-provider backend

- **Status:** draft (revision 2) → for approval, then `/tasks`
- **Spec:** [spec.md](spec.md)   **Date:** 2026-10-22 (revision 1: 2026-10-20)
- **Branch:** `features-2x` (2.x.x). **Fix vs feature:** feature. `README.md` and `docs/` are not touched; what 2.x.x says about itself stays in `specs/`.
- **Scope of this revision.** Revision 1 planned only the **backend half** (R4, R7–R10, R12), and it was written before the operator's answers of 2026-10-22. This revision plans **the whole spec**, R1–R12 and AC1–AC9. It folds in the decisions on Q2–Q7 and grounds every claim in the code as it stands on `features-2x` at 2.0.29. §0 records what changed and why. The researched provider facts of revision 1 (§3, §5, §8) are kept: they are still right.
- **Nothing here is implemented by this document.** The browser half described in §1 exists; the backend half does not. The drafts in [drafts/](drafts/) are still uncompiled.

## 0. What changed from revision 1, and why

| # | Change | Why |
|---|---|---|
| C1 | The plan covers the **control** too: R1–R3, the browser half of R5/R6, R11, AC1, AC5 and AC9. | The front half exists in `cobolt-web-player` (§1.1), but the model, IDE, System KB and vocabulary still have nothing for it. The spec is one feature; one plan has to carry it. |
| C2 | **Identity operations no longer travel over the WebSocket.** Every one is an HTTPS request to a closed set of `/auth/*` routes. Each route runs the matching generated `AUTH-*` program (D4, D5). This replaces revision 1's split: sign-in over the WebSocket, refresh over HTTPS through a session handle. It also replaces the player's `{"identity":…}` WebSocket frames (`cobolt-web-player/src/auth.rs` `wire::encode`), which the code itself labels "the player's proposal". | A WebSocket message cannot set a cookie. An access token and an `HttpOnly` refresh cookie can only be issued together in one HTTPS response. Q3 (decided 2026-10-22) confirms the HTTPS surface. The server's frame parser (`cobolt-wire/src/proto.rs` `parse_client`) knows no `identity` frame anyway. |
| C3 | **MFA covers TOTP, SMS and e-mail codes** (Q5, decided). The MFA screen says which kind it is and where the code went, and offers a choice when the provider asks for one. TOTP enrolment gets its own screen. | Today `Screen::Mfa` has one generic `code` field (`auth.rs`). No kind is modelled. |
| C4 | A **consent hook** for reCAPTCHA (Q6, decided): `RecaptchaConsent`, `onConsentRequired`, `GrantConsent`, `DenyConsent` (D10). | Today `dom/captcha.rs` `captcha_load` injects Google's script at the first protected action. There is no consent path. |
| C5 | A **form-level `RequiresSignIn` property** (Q4, decided; R11) (D11). | Absent: no `requires_sign_in` anywhere in `crates/`. |
| C6 | **Mobile** keeps the refresh credential in the OS secure store, behind the shell (Q7, decided). The shell's Rust side makes the identity requests, so the refresh token never enters page script (D6). | R5 says the refresh credential is not readable by page script. Revision 1 had the sign-in program return it to the page when `client = "native"`. That would break R5 on mobile. |
| C7 | `[auth]` (token validation) is **derived from `[identity]`** through OIDC discovery. | Today `config.rs` defaults `jwks_uri` to `<issuer>/.well-known/jwks.json`, which is right for Cognito only, and has no discovery. A developer should not configure the same issuer twice. |
| C8 | Two front-half behaviours change: `onSignUpCompleted` is raised **once**, and a temporary-password challenge gets its own **New password** screen. | Today `on_result` raises `SignUpCompleted` after SignUp→`NeedsConfirm` **and again** after Confirm→`Done`, and also when a *sign-in* answers `NeedsConfirm`. `NeedsNewPassword` lands on `Screen::Reset`, which asks for a code that case does not have (`auth.rs` `on_result`). |
| C9 | The bundle's **secret scan** stops flagging the control's label properties. | `cobolt-web-bundle/src/secrets.rs` `secret_property` reports any property whose name contains `password` and has a value, so `PasswordLabel="Senha"` would fail the Build. |
| C10 | The default flow of each adapter is recorded as a **working assumption to confirm**, no longer as a decision. | The spec keeps it open under Q1 ("what remains for `/clarify`"). Only "the flow is chosen per provider" is decided. |

## 1. What exists today (read 2026-10-22)

The spec's §1 is dated 2026-10-19 and is now stale in three places. JWT validation exists (`cobolt-server/src/jwt.rs`, `jwks.rs`). The browser half of the control exists. The `lib.rs:7266` citation has moved: the "tokens are kept by the runtime" comment is now at `cobolt-compiler/src/lib.rs:7541`. This plan does not edit the spec; whoever next edits the spec should refresh §1.

### 1.1 The browser half: `crates/cobolt-web-player`

- **`src/auth.rs`** is a sans-IO state machine. It is tested against `MockBackend` (19 tests, `auth.rs:993-1316`):
  - `Session` keeps the access token in memory only. Its `Debug` redacts the token. `token()` is `pub(crate)`, and `claim()` serves `GetClaim`.
  - `Screen` has `SignIn | SignUp | Confirm | Forgot | Reset | Mfa | SignedIn`. `Flow` has `InApp | Hosted`.
  - `Config::{sign_up, confirm, forgot, mfa, recaptcha_site_key, texts}`.
  - `BackendCall`, `AuthResult`, and `AuthEvent` with all six R3 events.
  - `Op::{Backend, Captcha, Notify, Message, Redirect}`.
  - `Machine::act` refreshes 60 s before expiry. `Machine::on_result` applies the generic-message rules of R10.
  - `Labels::read` reads 18 label properties. `config_from` reads `Mode`, `RecaptchaSiteKey`, `SignInFailedText`, `ResetRequestedText`, `ErrorText` and `ShowSignUp`/`ShowConfirm`/`ShowForgot`/`ShowMfa`.
  - `redirect_allowed` accepts https, or http to loopback.
  - `wire::{encode, decode}` encode the `{"identity":…}` proposal, with ids from `ID_BASE = 0x8000_0000`.
- **`src/view/auth.rs`** draws the screens. Every field is inside a `<label>`. The `autocomplete` values are `username` / `current-password` / `new-password` / `email` / `one-time-code`. The status line has `role="alert"` and the root has `aria-busy`. Focus goes to the first empty field only after the user has acted (10 tests).
- **`src/host.rs`** handles the methods in `auth_method` (`GETCLAIM`, `SIGNOUT`, `REFRESH`). It keeps one session for the whole application (`auth`, `auth_sync`), arms refresh with `auth_deadline`, and handles the hosted return in `auth_hosted_return`. Tests are in `host/auth_tests.rs` (12).
- **`src/tree.rs`** `sync_auth_props` publishes `State`, `UserName`, `Email` and `Mode`.
- **`src/dom/captcha.rs`** loads `api.js?render=<key>` and calls `grecaptcha.execute(siteKey, {action})`.
- **`src/api.rs`**: `PlayerConfig.{recaptcha_site_key, auth_flow, hosted_code, hosted_state}`.
- **`src/net.rs`** sends `{"type":"auth","token":…}` first, and on an expired token refreshes and repeats the call once.
- **`cobolt-web-bundle/assets/loader.js:690-715`** reads `?pr-code=…&pr-state=…` after a hosted return and strips them with `history.replaceState`.
- **`cobolt-web-bundle/examples/mock_backend.rs`** mocks the identity frames over a WebSocket. It was used for WASM-016 T5's Chrome check.

### 1.2 What the rest of the workspace has, and lacks

- **Model.** There is **no `ControlType::Authentication`**: `cobolt-forms/src/model.rs` `ControlType::ALL` at `:2926`. `from_str` turns `"Authentication"` into `Custom{plugin_id:"unknown", control_id:"Authentication"}` (`:3126-3137`). The writer (`xml.rs:2046`, `as_str` at `model.rs:3055-3058`) then saves it as `type="unknown:Authentication"`. The player finds it by name (`kinds.rs` `PlayerKind::Authentication`, class C).
- **Vocabulary.** `cobolt-ast/src/methods.rs` `is_known_method` lists `REFRESH` and `SIGNOUT` but **not `GETCLAIM`**. So `AUTH-1::GetClaim( "sub" )` parses as a subscript.
- **IDE.** There is no toolbox entry (`panels/toolbox.rs`), no Tr strings, and no identity settings. `panels/web_settings.rs` covers style, browsers, variants, isolation, the Backend address and exports.
- **System KB.** `cobolt-compiler/src/lib.rs` has no Authentication docs.
- **Server.**
  - `serve.rs` `serve_stream` (`:309-360`) routes `GET /health`, `GET /dl/<token>` and the WebSocket upgrade; everything else is 404. `http.rs` is hand-rolled on `httparse`, with `SECURITY_HEADERS`, **no POST body and no cookies**.
  - `jwt.rs` `TokenValidator` has the `cognito` / `keycloak` / `entra` / `generic` profiles (`check_application`, `check_access_token`), and `token_rules.rs` tests them.
  - `config.rs` has `IdentitySection { provider, flow, settings, recaptcha, attempts_per_minute }` and `RecaptchaSection { secret, min_score, hostnames, endpoint }` (`:188-215`). These are parsed, but `resolve()` only clones them: the secrets are not resolved, `provider` and `flow` are not validated, and **nothing reads them**.
  - `limits.rs` `KeyedLimiter` exists. `log.rs` `LogRecord` has no field that could hold a token.
  - `backend.rs` `Backend::load` reads `exports.toml` and `src/*.cbl`, non-recursively (`load_programs`). `ExportDecl.public` exists (`sig.public`). `exec.rs` `execute` runs a fresh `Interpreter` per call with `set_backend_hooks`.
- **Runtime.** `cobolt-runtime/src/interpreter/remote.rs` `BackendHooks { progress, emit, files, cancel_requested }` has no identity field. `builtins.rs` `BUILTINS` has no `AUTH-*` entries.
- **CLI.** `rcrun backend` has `run | describe | check | release | build` (`cobolt-cli/src/main.rs:543-610`). There is no `auth` subcommand.
- **Scaffold.** `cobolt-project-tools/src/create.rs` writes only `backend/src/health.cbl` (`BACKEND_STARTER`).
- **Dependencies.**
  - `cobolt-server` has `jsonwebtoken` (rust_crypto), `ureq` (native-tls), `sha2`, `base64` and `getrandom`.
  - The workspace has `hmac = "0.12"`, but `cobolt-server` does not use it.
  - `cobolt-server/tests/dependency_guards.rs` forbids C crypto (`NO_C_CRYPTO`) and asserts that `hyper` is absent.

## 2. Approach

```mermaid
flowchart LR
  subgraph Page["Browser page / Tauri web view"]
    AC["Authentication control\n(player auth.rs machine + view/auth.rs)"]
  end
  subgraph Shell["Mobile shell (WASM-012)"]
    SS["identity command\n+ OS secure store"]
  end
  subgraph BE["Backend (cobolt-server)"]
    R["/auth/* routes\n(closed table, CSRF + origin checks)"]
    P["generated AUTH-* programs\n(COBOL, public)"]
    H["COBOL::\"AUTH-OP\" -> BackendHooks.identity"]
    S["identity service\nadapters · reCAPTCHA · limits · timing floor"]
    V["TokenValidator (exists)"]
  end
  IDP["Cognito · Keycloak · Entra"]
  AC -- "fetch, credentials: include\n(web)" --> R
  AC -- "invoke (mobile)" --> SS -- "HTTPS, X-PR-Client: native" --> R
  R --> P --> H --> S --> IDP
  AC -- "wss + bearer access token\n(every other call, WASM-005)" --> V
```

1. **The control (R1–R3).** It becomes a real `ControlType::Authentication` (D1), available in the toolbox of Web and Mobile projects only. The player's existing machine and screens stay. They gain the MFA kinds (C3), the New password screen (C8), the consent hook (C4) and a six-language default text table (D12). The vocabulary is fixed in §4.
2. **reCAPTCHA (R4).** The player obtains a token per protected action, as today. The backend verifies it with `siteverify`, checking action, hostname and score, and treats each token as single-use (D9). The site key has one source, the backend configuration: it is served by `GET /auth/capabilities` and copied into the bundle at Build, so the CSP opens Google's origins only when it is set.
3. **Tokens (R5, R6).**
   - The access token stays in the player's memory, as today.
   - **On the web**, the refresh token goes in the response of the same HTTPS request that signed the user in, as a sealed `__Host-` cookie (`HttpOnly; Secure; SameSite=Strict; Path=/`). It is refreshed by `POST /auth/refresh`.
   - **On mobile**, the shell makes the requests and keeps the refresh token in the OS store (D5, D6).
   - No route ever puts a token in a URL. Logs carry no token field. `GetClaim` returns a claim.
4. **The identity backend (R7–R10).**
   - The generated public `AUTH-*` COBOL programs (D4) are run by the `/auth/*` routes through `exec::execute`.
   - Each program's body is one `COBOL::"AUTH-OP"` call, served by a new `BackendHooks.identity` that reaches the identity service: adapters, reCAPTCHA, limits, the timing floor and generic failures (D2, D7, D8).
   - Provider settings live in `backend/backend.toml` `[identity]`. They are edited by an IDE settings page with **Test connection** (D13).
5. **Flows (R8b).** Each adapter declares `flow()` and `capabilities()`. `GET /auth/capabilities` gives the control the effective flow and the screens it may offer. The hosted flow is brokered by the backend with authorisation code + PKCE (D5).
6. **Route protection (R11).** A form property `RequiresSignIn` makes the player show the sign-in form first, then the requested form with its parameters (D11). The backend's token requirement on non-public programs (`serve.rs` `on_call`) stays the actual security boundary.
7. **Assurance (R12).** One adapter-conformance suite runs against a mock of each provider (AC8). Browser checks go through the existing Chrome harness (AC1, AC5, AC7). A Keycloak-container run is gated on the operator authorising the image pull (§10).

## 3. What each product offers (researched 2026-10-20, kept from revision 1)

Source tags are in §8. **Bold UNVERIFIED** means the documentation fetched does not say. The adapters treat such facts as configuration, never as assumptions.

### 3.1 AWS Cognito user pools

| Question | Finding |
|---|---|
| In-app flow | Public operations need no AWS credentials [C1]. `InitiateAuth.AuthFlow`: `USER_AUTH` (choice-based), `USER_SRP_AUTH`, `USER_PASSWORD_AUTH`, `CUSTOM_AUTH`, `REFRESH_TOKEN_AUTH`, `REFRESH_TOKEN`; the app client must allow it (`ExplicitAuthFlows` `ALLOW_*`) [C2][C24][C26]. AWS recommends SRP for a custom-built password sign-in [C21]. A client with a secret needs `SECRET_HASH` on every call (HMAC-SHA256 of `username + client_id` keyed with the client secret) [C25]. |
| MFA | `ChallengeName` + `Session` come back; the answer is `RespondToAuthChallenge` with `ChallengeResponses`: `SMS_MFA_CODE`, `SOFTWARE_TOKEN_MFA_CODE`, `EMAIL_OTP_CODE`, `NEW_PASSWORD`, `ANSWER` (for `SELECT_MFA_TYPE`), always with `USERNAME` (+`SECRET_HASH`) [C3]. TOTP enrolment: `MFA_SETUP` → `AssociateSoftwareToken` → `VerifySoftwareToken` [C2][C22]. Session lifetime 3 min default (3-15) [C19][C24]. Email MFA needs SES and the Essentials/Plus plan [C22]. *Doc inconsistency:* the lists name both `EMAIL_MFA`/`EMAIL_MFA_CODE` and `EMAIL_OTP`/`EMAIL_OTP_CODE` [C3] — the adapter accepts both names and a live test must settle it. |
| Self-service (all public JSON, `ClientId` required) | `SignUp` {Username, Password, UserAttributes[{Name,Value}]} → {UserConfirmed, UserSub, CodeDeliveryDetails}; `ConfirmSignUp` {Username, ConfirmationCode}; `ResendConfirmationCode` {Username}; `ForgotPassword` {Username} (needs self-service recovery configured, else `InvalidParameterException`); `ConfirmForgotPassword` {Username, ConfirmationCode, Password} [C4]-[C8]. |
| Refresh / sign-out | `InitiateAuth` `REFRESH_TOKEN_AUTH` {`REFRESH_TOKEN`} — **unavailable when refresh-token rotation is on**, where `GetTokensFromRefreshToken` is used [C9]. `RevokeToken` {ClientId, ClientSecret, Token}; `GlobalSignOut` takes the user's access token. A revoked JWT still passes offline signature/exp checks — check `origin_jti` revocation through Cognito if it matters [C10][C11]. |
| Token facts | `iss` = `https://cognito-idp.{region}.amazonaws.com/{poolId}` (an "updated issuer" `https://issuer-cognito-idp…` exists; read `issuer` from discovery) [C13]. JWKS `…/.well-known/jwks.json`. RS256; **access and ID tokens are signed with different keys** [C11][C12]. **Access token:** `token_use`="access", `client_id`, **no `aud`**, `scope`, `cognito:groups`, `username`, `origin_jti`. **ID token:** `token_use`="id", `aud`=client id. API sign-in tokens carry only the scope `aws.cognito.signin.user.admin` [C12]. Validate `exp`, `iss`, `token_use`, `client_id` [C11]. |
| Hosted flow | `/oauth2/authorize` (PKCE optional, S256 only, code valid 5 min), `/oauth2/token` (`client_secret_basic`/`client_secret_post`), `/logout`, `/oauth2/revoke` [C14]-[C17]. Redirect URI: absolute, pre-registered, https except `http://localhost` [C21]. Access token 1 h default (1 s-1 day), refresh 30 days default (60 min-10 y), rotation optional with ≤60 s grace [C9][C24]. Third-party IdP sign-in only through managed login [C19]. |
| Limits / enumeration | Per-Region quotas: UserAuthentication 120 RPS, UserCreation 50, UserAccountRecovery 30 (not adjustable), UserResourceRead 50 [C18]. Lockout after 5 failed password attempts, 1 s doubling to ~15 min (documented as subject to change) [C19]. `PreventUserExistenceErrors`: API default LEGACY (returns `UserNotFoundException`), console default ENABLED; `SignUp` always returns `UsernameExistsException` [C20]. |

### 3.2 Keycloak (stable 26.8.1 read)

| Question | Finding |
|---|---|
| In-app flow | Direct Access Grants (ROPC) exists per client [K1], but the guide says per RFC 9700 it **"MUST NOT be used"**, it is removed from OAuth 2.1, and it supports **no** self-registration, required actions or social login [K1]. Default for new clients **UNVERIFIED** (an old upgrade note says disabled [K4]). No MFA challenge/response API is documented (community: an OTP is a `totp` form parameter, a missing one gives the same 401 [K7] — **UNVERIFIED** officially). |
| Self-service | Registration and "Forgot password" are realm toggles that enable **browser pages** [K2]; there is **no public registration or reset JSON API**. The Admin REST API (`/admin/realms/{realm}`) can `POST /users`, `PUT /users/{id}/reset-password`, `PUT /users/{id}/execute-actions-email` (body: required-action strings; query `client_id`, `redirect_uri`, `lifespan`), `PUT …/send-verify-email`, `POST …/logout` [K3] — using a confidential **service-account client** (`client_credentials`) with `realm-management` roles (`manage-users`, `view-users`, `query-users`); the per-endpoint role mapping is **UNVERIFIED** in the reference [K2]. |
| Refresh / sign-out | Token endpoint `grant_type=refresh_token`; rotation = "Revoke Refresh Token" [K2]. `/protocol/openid-connect/revoke` accepts access and refresh tokens [K1]; the direct `logout` with a refresh token is legacy [K1]. |
| Token facts | Discovery `/realms/{realm}/.well-known/openid-configuration`, JWKS `/realms/{realm}/protocol/openid-connect/certs` [K1]. Default signing algorithm **UNVERIFIED** (read `alg` from the JWKS). `typ` and `azp` cannot be removed from lightweight tokens [K2]; the `typ` values (Bearer/ID) are **UNVERIFIED** in the official docs (the validator's `Bearer` rule is the `keycloak` profile of `jwt.rs` `check_access_token`). `aud` is **not** the API by default: add an Audience mapper; `azp` = the client that obtained it [K2]. Roles `realm_access.roles`, `resource_access.{client}.roles` [K2]. |
| Hosted flow | `auth`, `token`, `logout`, `revoke`, `token/introspect` under `/realms/{realm}/protocol/openid-connect/` [K1]. PKCE is a per-client setting (S256/plain), not enforced unless set [K2]. Redirect URIs exact match, wildcard only at the end [K1][K2]. SSO Session Max 10 h default [K2]. |
| Limits / enumeration | Brute-force detection **disabled by default**; when on: permanent mode 30 failures, temporary 1 min wait increment up to 15 min [K2]. A locked user sees the same "Invalid username or password" [K2]. No `PreventUserExistenceErrors` equivalent documented; forgot-password behaviour **UNVERIFIED** [K2]. |

### 3.3 Microsoft Entra

| Question | Finding |
|---|---|
| In-app flow, workforce | Only **ROPC** (`POST {tenant}/oauth2/v2.0/token`, `grant_type=password`): tenants only, **blocked for MFA users, passwordless and federated accounts**; Microsoft says "do not use"; not removed [E1]. No in-app MFA challenge for workforce. |
| In-app flow, External ID (external tenants) | **Native authentication** API, GA announced 2024-09-30 [E4] (email-OTP/SMS second-factor GA date 2026-03-09 is from a search snippet only — **UNVERIFIED**). App registration with "Enable native authentication"; **no CORS, so only our backend can call it** [E2]; base `https://{subdomain}.ciamlogin.com/{subdomain}.onmicrosoft.com`; methods: email+password (with SSPR) and email OTP [E2]. If the app cannot handle a required method the API answers 200 with `challenge_type="redirect"` and the backend must fall back to the hosted flow [E2]. |
| Native endpoints [E2] | sign-up `/signup/v1.0/{start,challenge,continue}`; sign-in `/oauth2/v2.0/{initiate,challenge,token,introspect}`; reset `/resetpassword/v1.0/{start,challenge,continue,submit,poll_completion}`. Every call returns a `continuation_token` for the next. MFA: `/token` → `invalid_grant` suberror `mfa_required` → `/introspect` (methods) → `/challenge` → `/token` `grant_type=mfa_oob`; `registration_required` when none registered. TOTP not listed; the page is inconsistent on SMS vs email-only [E2]. |
| Graph, for an admin path | `POST /users` needs app permission `User.ReadWrite.All` [E16]; password reset by app-only `PATCH` needs `User-PasswordProfile.ReadWrite.All` [E17]; `authenticationMethod: resetPassword` is **delegated-only** [E18]; `revokeSignInSessions` needs `User.RevokeSessions.All` [E19]. |
| Token facts | Issuers: v1 `https://sts.windows.net/{tid}/`; v2 `https://login.microsoftonline.com/{tid}/v2.0`; external `https://{tenantid}.ciamlogin.com/{tenantid}/v2.0` [E5][E8]. Take `jwks_uri` from the metadata of the token's `ver` [E5][E11]; check `tid` against the issuer [E5]. RS256. `aud` v2 = the API's client id, v1 = client id or resource URI; `appid` (v1) / `azp` (v2) [E6]. **No `token_use`-style claim**: accept only a token whose `aud` is your own application, never an ID token; require `scp` or `roles`; a Graph access token must not be validated by third parties [E5][E7]. Roles: `scp`, `roles`, `groups` (overage → `hasgroups`) [E6]. |
| Hosted flow | `https://login.microsoftonline.com/{tenant}/oauth2/v2.0/{authorize,token,logout}` [E11] (no revocation endpoint in the OIDC table). PKCE recommended for all clients and required for SPAs (S256 recommended, plain allowed); auth code ~1 min [E9]. Redirect URI case-sensitive, ≤256 chars, http only for localhost [E12]. Refresh 90 days (24 h for SPAs and the email-OTP flow), new refresh token each use, old ones not revoked [E10]. Access token 60-90 min random default, configurable 10 min-1 day [E13]. External-tenant hosted URLs **UNVERIFIED**. |
| Limits / enumeration | Smart lockout: 10 failures in the public cloud, then 1 min, growing [E14]. Native API: double the polling interval on 429 [E2]. Distinct error codes `AADSTS50034` (user not found) / `AADSTS50126` (bad password) [E15] — **the adapter must normalise them**; no `PreventUserExistenceErrors` setting documented. |

## 4. The control's vocabulary (R3, fixed here)

Each item is registered in `ControlType::Authentication`'s event list (`cobolt-forms`), in `is_known_method` (`cobolt-ast/src/methods.rs`) where it is a method, and in the System KB tables (`cobolt-compiler/src/lib.rs`). Every item marked *exists* is already implemented by the player (§1.1).

| Kind | Name | Meaning |
|---|---|---|
| Event | `onSignedIn` · `onSignedOut` · `onPasswordResetRequested` · `onMfaRequired` | *exists*. No argument (WASM-ABI-v1 §12.2). |
| Event | `onSignUpCompleted` | *exists*; **changed (C8)**: raised once, when the account is usable. That is after `Confirm` succeeds, or straight after `SignUp` when the provider needs no confirmation. |
| Event | `onError` | *exists*; its argument is `class/code`, never a message (ABI §12.2). |
| Event | `onConsentRequired` | **new (D10)**: a protected action needs reCAPTCHA and consent has not been given. |
| Property | `State` | *exists*: `SignedIn` / `SignedOut`. Read-only. |
| Property | `UserName` · `Email` | *exists*. Read-only. `UserName` comes from `cognito:username`, else `preferred_username`. |
| Property | `Mode` | *exists* (run time: `InApp` / `Hosted`, the effective flow). Design time: `Auto` (the provider's flow, default) / `InApp` / `Hosted`. Check refuses `InApp` for a provider whose capabilities have no in-app sign-in. |
| Property | `ShowSignUp` · `ShowConfirm` · `ShowForgot` · `ShowMfa` | *exists*: the developer's choice of screens (R1). |
| Property | the 18 label/caption properties of `Labels::read`, and `SignInFailedText` · `ResetRequestedText` · `ErrorText` | *exist*; their defaults become six-language (D12). |
| Property | `RecaptchaSiteKey` | *exists*: an override. Its default comes from the backend's `[identity.recaptcha] site_key` (D9). Check warns when the two differ. |
| Property | `RecaptchaConsent` | **new (D10)**: `Implied` (default: today's behaviour) / `Ask`. |
| Method | `SignOut` · `Refresh` | *exist* (`host.rs` `auth_method`). |
| Method | `GetClaim( name )` | *exists*; **`GETCLAIM` is added to `is_known_method`**, so the inline form parses as a call. |
| Method | `GrantConsent` · `DenyConsent` | **new (D10)**. |
| Form property | `RequiresSignIn` | **new (D11)**: on `Form`, saved in `.cfrm` as `requires-sign-in="true"` (written only when true). |

## 5. Decisions

| # | Decision | Why | Rejected |
|---|---|---|---|
| **D1** | **A real `ControlType::Authentication`** in `ControlType::ALL`, with its event list, default size and toolbox entry, shown for **Web and Mobile** projects only (R1). `from_str` maps both `"Authentication"` and the legacy `"unknown:Authentication"` to it, so a form saved by today's code reloads as the real type and is saved back as `type="Authentication"`. `kinds.rs` finds it by the variant instead of by name. The tests that pin 62 types and 24 of class C (`kinds.rs:223`, `:232`) are updated as `WASM-004` plan.md:40 foresaw. | AC9 requires a `.cfrm` round-trip and the `ControlType::ALL`-driven tests. A `Custom` control has the default event `onClick` only (`model.rs:4147`), so the Designer could not bind the six events. | Keeping `Custom` by name: no events in the Designer, and the saved type text changes on save. |
| **D2** | **One adapter interface**, `IdentityProvider` (in the drafts' `mod.rs` shape): `sign_in`, `respond_to_challenge`, `sign_up`, `confirm_sign_up`, `resend_code`, `forgot_password`, `confirm_forgot_password`, `refresh`, `sign_out`, `start_hosted`, `finish_hosted`, `flow()`, `capabilities()`. Each result type already carries the **generic** failure form (R10): an adapter cannot return "no such user". `SignIn::Challenge` carries the **MFA kind** (`Totp`, `Sms{masked destination}`, `Email{masked destination}`, `Choice(kinds)`, `TotpSetup{secret, otpauth_uri}`, `NewPassword`) (C3). | R8: a fourth provider is an adapter, not a change to the control. | One code path per provider in the routes. |
| **D3** | **The token the frontend sends is the provider's own access token** (Q2, decided 2026-10-22), validated by the existing `TokenValidator`. The backend mints nothing. `[auth]` is **derived from `[identity]`** (C7): issuer and `jwks_uri` come from OIDC discovery (`/.well-known/openid-configuration`, fetched through `jwks.rs` `HttpFetcher` and its `address_allowed`), `client_ids`/`audiences` come from the client id, and `profile` from the provider. An explicit `[auth]` still wins, for a backend that only validates. | No key custody. The issuer's key rotation just works. One place to configure. | A backend-issued session token (the spec's "later option"). |
| **D4** | **The identity programs are generated public COBOL programs** (R7): `AUTH-SIGN-IN`, `AUTH-SIGN-UP`, `AUTH-CONFIRM`, `AUTH-RESEND`, `AUTH-FORGOT`, `AUTH-RESET`, `AUTH-MFA`, `AUTH-MFA-SETUP`, `AUTH-REFRESH`, `AUTH-SIGN-OUT`, `AUTH-START-HOSTED`, `AUTH-FINISH-HOSTED`, `AUTH-CAPABILITIES`. Each takes `AUTH-REQUEST` and returns `AUTH-RESULT`. These are groups in a generated copybook, `AUTH-GROUPS.cpy`, using the drafts' member names (`ONE-TIME-CODE`, `OUTCOME-TEXT`, …). Each body is one inline built-in, `COBOL::"AUTH-OP" ( "<OP>" AUTH-REQUEST AUTH-RESULT )`, registered in `builtins.rs` `BUILTINS` and served by a new `BackendHooks.identity: Option<Arc<dyn IdentityHook>>` (`interpreter/remote.rs`). The secrets and the code exchange therefore never sit in COBOL, while the mapping, the limits and the log are those of any program. **Where they live:** `backend/generated/identity/` holds the `*.cbl` files, the copybook and an `exports.toml` fragment (`public = true`). All carry the generated-code banner and are regenerated by Build / Run / Check of a project with an `[identity]` section. `Backend::load` also reads that folder and **merges** the fragment; a developer export with the same name is an error. The developer's `backend/exports.toml` and `backend/src/` are never written. **Transport:** these programs are run **only by the `/auth/*` routes** (D5). They are absent from the WebSocket's export table, so the refresh token a program produces never crosses a channel that cannot carry it safely. | R7 asks for programs. Keeping the provider conversation in Rust keeps secrets out of COBOL and out of the debugger's variable view. Never writing the developer's files follows "user code is sacred". | Writing into `backend/src/` and `exports.toml` (developer files). The programs as ordinary WebSocket exports (they could not set the cookie, C2). A Rust-only identity service with no COBOL (contradicts R7). |
| **D5** | **The HTTPS surface is a closed route table** mounted beside `/health` and `/dl/` in `serve.rs` `serve_stream`. The handlers are transport-neutral (`fn(AuthRequest) -> AuthResponse`), so they move unchanged if WASM-003 changes the HTTP layer (§9 R3). `http.rs` gains a bounded body reader (16 KiB) and cookie parsing and emission. **Routes:** `POST /auth/{sign-in, sign-up, confirm, resend, forgot, reset, mfa, mfa-setup, refresh, sign-out, hosted/finish}`, `GET /auth/{capabilities, hosted/start, hosted/callback}`. **On every POST:** `Content-Type: application/json` and the custom header `X-PR-Identity: 1`, which force a CORS preflight and so stop cross-site forms (WASM-013 R6). The `Origin` must be in an explicit allow-list (`[identity] origins`, defaulting to the Web bundle's origin), and CORS answers that origin with credentials, never `*`. `SECURITY_HEADERS` and `Cache-Control: no-store` are sent on every response. **Cookies:** `__Host-pr_refresh` (`HttpOnly; Secure; SameSite=Strict; Path=/`; the `__Host-` prefix requires `Path=/`) holds the **sealed** refresh token and the user's name (Cognito's `SECRET_HASH` needs it). `__Host-pr_hosted` (`SameSite=Lax`, 10 min) holds the sealed PKCE verifier, nonce, backend `state` and return address of a hosted sign-in in flight. **Hosted flow:** the player navigates to `GET /auth/hosted/start?return=<page>&state=<player state>`. The return address's origin must be in the allow-list. The backend sets `__Host-pr_hosted` and answers 302 to the provider (S256). At `GET /auth/hosted/callback` it checks `state` against the cookie, exchanges the code with the verifier (and the client secret, which only the backend holds), sets `__Host-pr_refresh`, and answers 302 to the return address with `?pr-code=<handle>&pr-state=<player state>`. That is the shape `loader.js` already reads, so **no JavaScript changes**. The handle is 32 random bytes, single-use and valid 60 s. It refers to this sign-in's access token in the backend's memory and carries no token. `POST /auth/hosted/finish {handle}` returns the access token; when the handle is unknown (another instance, or expired) it falls back to refreshing from the cookie. The player checks `pr-state` against the value it kept before leaving (a random value, not a token) (R8b). | A WebSocket message cannot set a cookie (C2). Q3 is decided. No URL carries a token (R6). The hosted return keeps `loader.js` as it is (PRIME DIRECTIVE). | A session handle that binds the cookie after a WebSocket sign-in (revision 1): a handle readable by page script could be redeemed in another browser. A raw provider refresh token in the cookie (D7 explains the sealing). Changing `loader.js`. |
| **D6** | **Mobile (Q7, decided 2026-10-22):** in the Tauri shell the player does not call `/auth/*` itself. It invokes the shell's **identity command** (a WASM-012 host-API entry), whose Rust side makes the same HTTPS requests with `X-PR-Client: native`. The backend then answers with the refresh token **in the body** instead of a cookie, and the shell stores it in the OS secure store (Keychain / Keystore, WASM-012 R9) and returns only the access token and claims to the page. Biometric unlock, when the project enables it, is the store's user-presence condition on reading that item. On mobile the hosted flow uses the system browser and a deep link (WASM-012 R9) carrying the same single-use handle. **This needs WASM-012 to provide the command, the store and the deep link.** | R5: "the operating system's secure store on mobile", and not readable by page script. A Tauri web view's origin is cross-site to the backend, so `SameSite` cookies do not apply there. | Returning the refresh token to the page when `client = "native"` (revision 1 D5) — page script could read it. |
| **D7** | **Sealing:** both cookies and the in-memory handle table use AEAD (XChaCha20-Poly1305 from the RustCrypto `chacha20poly1305` crate, pure Rust, which passes `NO_C_CRYPTO`). The key comes from `[identity] cookie_key` (`env:` or `file:`, 32 bytes). **There is no plaintext fallback:** a missing key stops the backend with a message (WASM-013 R8). The cookie carries an issue time and the backend refuses a cookie older than the provider's refresh lifetime. | A refresh token that leaks from a proxy's header log stays unusable. The hosted cookie needs integrity (the verifier and the return address). | HMAC only: integrity without confidentiality for a long-lived credential. Server-side session storage: state in a backend the series keeps stateless (WASM-INDEX D4). |
| **D8** | **Generic failures, limits, constant-ish timing (R10), kept from revision 1.** Every sign-in failure is `AUTH-FAILED` ("the user name or password is not correct"). Every reset request answers `AUTH-RESET-REQUESTED`, whatever the provider said. A failed sign-in is padded to the median time of successful ones, with a 250 ms floor. Rate limits are the backend's own, **per source address and per account** (the account key is a SHA-256 of the lower-cased user name, never the name itself), 10 attempts a minute by default (`attempts_per_minute`). They reuse `limits.rs` `KeyedLimiter` and add to the provider's lockout (§3), which is relied on, not reimplemented. Passwords and codes never enter a `LogRecord`; the type has no field for them. | AC3. AC7. | |
| **D9** | **reCAPTCHA v3 (R4), kept from revision 1 with one change.** The backend verifies with `siteverify`, the secret only in `[identity.recaptcha] secret`. It checks **action** (`sign_in` / `sign_up` / `forgot_password`, the names in `auth.rs` `guarded`), **hostname** (`hostnames`) and **score** (`min_score`, default 0.5). A token is single-use: the backend remembers accepted tokens for 2 minutes. "Over free quota" is a failure (§6). A refusal answers the same generic message as a wrong password. **Change:** the public `site_key` sits in `[identity.recaptcha]` too. Build copies it into `BundleOptions.recaptcha_site_key` (`cobolt-web-bundle/src/lib.rs:154`, which opens the CSP's reCAPTCHA origins), and `/auth/capabilities` serves it, so one setting enables both halves. `CreateAssessment` (Enterprise) is not implemented; `CaptchaVerifier` is its seam. | Today `BundleOptions.recaptcha_site_key` is set by nothing outside its crate (only by `examples/fixture_bundle.rs`). A backend requiring reCAPTCHA with no site key in the page would refuse every request. | A site key configured separately in the control and in the backend. |
| **D10** | **The consent hook (Q6, decided 2026-10-22).** `RecaptchaConsent = Ask` makes the control **not** load Google's script (`dom/captcha.rs` `captcha_load`) until consent is given. At the first protected action it raises `onConsentRequired` and holds the action. The handler shows the developer's own consent text and calls `AUTH-1::GrantConsent` (the action proceeds) or `AUTH-1::DenyConsent` (the action fails with the generic message and `onError` `consent/declined`). Consent holds for the page's lifetime. Remembering it across visits is the developer's (a `Locale`-like application property is not invented here). `Implied`, the default, keeps today's behaviour. | A hook, as decided, leaves the legal text and the regional rules (LGPD, GDPR) with the developer. Changing the default to `Ask` would silently break projects that set no consent handler. | A built-in consent banner with our own legal text. |
| **D11** | **Route protection (Q4, decided 2026-10-22; R11).** `Form.requires_sign_in: bool` (`cobolt-forms/src/model.rs`, next to `form_format`). In `xml.rs`, the attribute is read beside `form-format` (`:423`) and written only when true (beside `:1813`). The Properties panel shows it for Web and Mobile forms. The **sign-in form** is the project's `SignInForm` web setting; by default it is the first form that holds an Authentication control. Check fails when a form requires sign-in and there is no sign-in form. **Player:** opening (or deep-linking to) a protected form while signed out opens the sign-in form instead and remembers the request (name and parameters, WASM-011 R6). After `onSignedIn` it opens the requested form; after `onSignedOut` a protected form open in the SPA is closed and the sign-in form is shown. The developer's `onSignedIn` handler still runs first. | A form-level property, as decided. It is UX, not security: the backend's token check on non-public programs (`serve.rs` `on_call`) is the boundary, and the plan says so in the KB. | A shell property (spec 049); an explicit handler check. |
| **D12** | **Six-language defaults.** The control's default end-user texts (the 18 labels and the 3 messages) move from English literals in `auth.rs` (`Labels::read`, `Texts::default`) to one table in `cobolt-forms` (`auth_texts.rs`, EN/ES/PT/JA/ZH/FR). The player picks the row by the application's `Locale` (ABI §12.1), and the Designer paints with the same table. The developer's own property values always win (R1: "their text is translatable"). IDE strings (toolbox name, property help, the settings page, Test connection results) are `Tr` fields in `crates/cobolt-ide/src/i18n.rs`, six languages. | i18n ×6 (tech.md). One table for both paint paths (designer `paint.rs`, player). | Per-crate copies of the texts. |
| **D13** | **Settings page and Test connection (R9).** An **Identity** section of the project's Web/Mobile settings (`panels/web_settings.rs`) edits `backend/backend.toml` `[identity]`: provider (Cognito / Keycloak / Entra workforce / Entra External ID), the provider's fields, flow (`Auto` by default), allowed origins, the reCAPTCHA site key, secret and threshold, and `cookie_key`. **Secret fields accept only `env:NAME` or `file:PATH`**; a literal is refused in the page, and `config.rs` `resolve()` refuses one in release mode, as it already refuses `unsafe_*`. `resolve()` gains what it lacks today: it resolves `[identity]` secrets through `resolve_secret`, validates `provider` and `flow`, and checks the settings each provider needs. **Test connection** runs `rcrun backend auth test`, which prints one line per check and never a value: discovery reachable and its `issuer` matching; JWKS has signing keys; for an in-app Cognito client, `InitiateAuth` for a random non-existent user answers the expected refusal (proving the client id and `SECRET_HASH`); for hosted, the authorise endpoint answers; reCAPTCHA `siteverify` with a dummy response answers `invalid-input-response` (secret valid), not `invalid-input-secret`. | R9 ("never written to a form, a copybook or any frontend file"). The project's no-credentials-in-commits rule: `backend.toml` lives in the project folder and is committed. | Storing secret values in `backend.toml`. An IDE keychain (none exists in the workspace). |
| **D14** | **Secret scanning (AC6, C9).** `secret_property` (`cobolt-web-bundle/src/secrets.rs`) also exempts names ending in `label`, `caption`, `text` and `link`, so the control's own properties do not fail the Build. Build adds an **exact-value scan**: it resolves the `[identity]` secrets and the `cookie_key`, then searches the bundle, the forms and the copybooks for each value, failing with file and setting name and never printing the value. This is needed because a reCAPTCHA secret and a site key share a shape (`6L…`), so a pattern cannot tell them apart. | AC6. WASM-013 R7 owns the pattern list; this adds what only this spec can know. | |
| **D15** | **Interpreter/binary parity.** `COBOL::"AUTH-OP"` runs only where `BackendHooks.identity` is set, that is, in the backend (`exec.rs`, both `rcrun backend run` and the built backend binary, which share `cobolt-server`). Anywhere else (desktop `rcrun run-form`, embedded child forms, the compiled desktop binary) it is the same run-time error, "identity operations run in the Backend". The browser generator (`cobolt-wasm`) does not list it among the browser-capable built-ins, so Check refuses it in a frontend program. | interpreter-binary-parity skill. One runtime, one rule. | |
| **D16** | **Front-half behaviour fixes (C8).** `onSignUpCompleted` is raised once (§4). A sign-in answered `NeedsConfirm` goes to the Confirm screen without raising it. `NeedsNewPassword` (Cognito `NEW_PASSWORD_REQUIRED`) goes to a new `Screen::NewPassword` (fields: `new-password` twice), not to `Reset`. `NeedsMfa` carries the kind (D2). The MFA screen names the kind and the masked destination. A choice gives the Choose screen, and TOTP set-up gives the `MfaSetup` screen (the `otpauth://` link and the secret as text, plus a QR code drawn as DOM nodes from the `qrcode` crate, pure Rust; §9 R8). | The machine's own tests stay and grow (§7). | |

## 6. The capability table the control reads (R8, R8b)

`Y` means supported in the flow; `-` means not supported; "hosted" means the control (and the backend) uses the provider's page for that operation. `GET /auth/capabilities` serves this table's column for the configured provider, so the control shows only what is possible (`Machine::allowed` already hides in-app screens for `Flow::Hosted`).

| Operation | Cognito | Keycloak | Entra workforce | Entra External ID |
|---|---|---|---|---|
| **Default flow** (working assumption, Q1 — to confirm) | in-app | hosted | hosted | hosted; `native` opt-in |
| Sign in with password | Y (`USER_PASSWORD_AUTH`; SRP when the client forbids it — **not in this cut**, §10) | hosted | hosted | Y in `native` (`/initiate`+`/challenge`+`/token`) |
| Sign up | Y (`SignUp`) | hosted | hosted (admin Graph path only) | Y in `native` (`/signup/*`) |
| Confirm (e-mail/phone code) | Y (`ConfirmSignUp`, `Resend…`) | hosted | hosted | Y in `native` (`/signup/continue`, `oob`) |
| Forgot / reset password | Y (`ForgotPassword`, `ConfirmForgotPassword`) | hosted | hosted | Y in `native` (`/resetpassword/*`) |
| New password at first sign-in | Y (`NEW_PASSWORD_REQUIRED`) | hosted (required action) | hosted | hosted |
| MFA: TOTP (Q5) | Y (`SOFTWARE_TOKEN_MFA`) | hosted | hosted | **-** → hosted (`challenge_type=redirect`) |
| MFA: SMS code (Q5) | Y (`SMS_MFA`) | hosted | hosted | **UNVERIFIED** in `native` [E2] → hosted when refused |
| MFA: e-mail code (Q5) | Y (`EMAIL_OTP`; needs SES and the Essentials/Plus plan) | hosted | hosted | Y in `native` (email OTP) |
| MFA enrolment | TOTP in-app (`MFA_SETUP`); SMS / e-mail use the verified phone / e-mail attribute, with no in-app enrolment screen | hosted | hosted | **-** → hosted |
| Refresh | Y (`REFRESH_TOKEN_AUTH`; `GetTokensFromRefreshToken` when rotation is on) | Y (token endpoint) | Y (token endpoint) | Y (v2.0 token endpoint [E9]) |
| Sign-out / revoke | Y (`RevokeToken`, `GlobalSignOut`) | Y (`revoke`, `logout`) | logout only (no revoke endpoint) | logout only |
| Social / federated | **-** → hosted (managed login) | hosted | hosted | **-** → hosted |
| Passkeys / WebAuthn | **-** (out of scope, WASM-031) | **-** | **-** | **-** |
| Enumeration protection | provider setting `PreventUserExistenceErrors=ENABLED` **and** the adapter's normalisation | adapter normalisation | adapter normalisation (`AADSTS50034`→generic) | adapter normalisation |

**reCAPTCHA v3 (researched 2026-10-20, kept):**
- **Classic verification** (what R4 names): `POST https://www.google.com/recaptcha/api/siteverify` (or `www.recaptcha.net`) with `secret`, `response` and an optional `remoteip` [R1]. The answer carries `success`, `score` (0.0-1.0), `action`, `challenge_ts`, `hostname` and `error-codes` [R1][R2]. A token is valid **2 minutes** and verifiable **once** [R1]. The suggested threshold is 0.5 [R2].
- **Status in 2026:** the classic pages are marked deprecated, and **classic keys can no longer be created**. Existing keys keep working and are being migrated into Google Cloud projects; Google steers new integrations to `CreateAssessment` [R1][R3][R4][R5].
- **A fail-open trap** [R1]: without billing, over the free quota (10,000 assessments a month [R5]), `siteverify` answers **HTTP 200, `success: true`, score 0.9 and "Over free quota."** The verifier treats that answer as a **failure** (`recaptcha-quota`).
- **`CreateAssessment`** is not implemented (`POST https://recaptchaenterprise.googleapis.com/v1/projects/{project}/assessments?key=…` [R6][R7]): it needs a Cloud project and an API key the operator must create.

## 7. Affected crates / files

| Crate / file | Change | R / AC |
|---|---|---|
| `cobolt-forms/src/model.rs` | `ControlType::Authentication` in `ALL`, `as_str`/`from_str` (+ the legacy `unknown:Authentication`), default size, event list; `Form.requires_sign_in` | R1, R3, R11, AC9 |
| `cobolt-forms/src/xml.rs` | `requires-sign-in` attribute; the round-trip test of the control | AC9 |
| `cobolt-forms/src/auth_texts.rs` (new) | six-language default texts | R1, AC1 |
| `cobolt-forms` designer paint (`paint.rs`) | the Designer draws the Sign in screen, or the single Hosted button, from the same texts | R1 |
| `cobolt-ast/src/methods.rs` | `GETCLAIM`, `GRANTCONSENT`, `DENYCONSENT` in `is_known_method` | R3 |
| `cobolt-web-player/src/auth.rs` | `wire` replaced by HTTPS request/response types (C2); MFA kinds, `NewPassword`, `MfaSetup`, Choose screens; the C8 event fixes; consent state | R1, R3–R5, R8b |
| `cobolt-web-player/src/view/auth.rs` | the new screens (autocomplete `one-time-code` / `new-password`, labels, live region) | R1, R2 |
| `cobolt-web-player/src/api.rs`, `host.rs` | identity requests through `fetch` with `credentials: "include"` and `X-PR-Identity` (web), or the shell command (mobile); the `ID_BASE` WebSocket path for identity is removed; `auth_deadline` unchanged | R5, R6 |
| `cobolt-web-player/src/dom/captcha.rs` | no script injection before consent when `RecaptchaConsent = Ask` | R4, Q6 |
| `cobolt-web-player/src/kinds.rs`, `tree.rs` | find the control by its variant; `RequiresSignIn` navigation (D11) | R11 |
| `cobolt-web-bundle/src/secrets.rs`, `src/lib.rs` | label exemption; exact-value scan; `connect-src` includes the backend's https origin | AC6 |
| `cobolt-web-bundle/examples/mock_backend.rs` | speaks the `/auth/*` routes instead of the identity frames | AC1, AC5 |
| `cobolt-server/src/identity/` (new; from `drafts/`) | `mod.rs` (trait, D2), `cognito.rs`, `keycloak.rs`, `entra.rs`, `oidc.rs` (discovery, PKCE S256), `recaptcha.rs`, `ops.rs` (generic failures, timing floor, limits), `seal.rs` (D7), `routes.rs` (D5), `hook.rs` (`IdentityHook` for `AUTH-OP`) | R4, R7–R10 |
| `cobolt-server/src/http.rs`, `serve.rs` | body reader, cookies, CORS with credentials, the route table mounted in `serve_stream` | R5, Q3 |
| `cobolt-server/src/config.rs` | `[identity]` resolution and validation; `origins`, `cookie_key`, `site_key`; `[auth]` derived through discovery (D3) | R9 |
| `cobolt-server/src/backend.rs` | load and merge `backend/generated/identity/` (D4) | R7 |
| `cobolt-server/Cargo.toml`, workspace `Cargo.toml` | `chacha20poly1305` (D7); `hmac.workspace = true` (Cognito `SECRET_HASH`) | |
| `cobolt-runtime/src/interpreter/remote.rs`, `builtins.rs` | `BackendHooks.identity`; `AUTH-OP` in `BUILTINS`; the D15 error | R7 |
| `cobolt-codegen/src/identity.rs` (new) | generates the `AUTH-*` programs, `AUTH-GROUPS.cpy` and the exports fragment (banner, deterministic) | R7 |
| `cobolt-cli/src/main.rs` | `rcrun backend auth test` | R9 |
| `cobolt-project-tools/src/create.rs` | a new Web/Mobile project's backend gets an `[identity]` stub (commented, `env:` references) and the regenerate hook (WASM-003 R22) | R7 |
| `cobolt-ide/src/panels/toolbox.rs`, `panels/properties.rs`, `prop_help_data.rs`, `panels/web_settings.rs`, `i18n.rs` | toolbox entry (Web/Mobile), property help, the Identity section and Test connection, Tr ×6 | R1, R9 |
| `cobolt-wasm` | `AUTH-OP` is not browser-capable (Check error) | D15 |
| `cobolt-compiler/src/lib.rs` doc tables + `assets/knowledge/chunked.data` | the control's events, properties and methods, `RequiresSignIn`, `AUTH-OP` (generated-only), the capability table, the security note of D11 | System KB, AC9 |
| `specs/WASM-ABI-v1.md` §12 | an amendment row: the Authentication control's identity requests leave the WebSocket (A8's `0x80000000` ids are no longer used for identity), plus `GrantConsent` / `DenyConsent` / `onConsentRequired` in §12.1/§12.2 | C2 |
| `specs/WASM-009-…/drafts/` | moved into `cobolt-server/src/identity/` when that module compiles, then removed | |

`README.md` and `docs/` are not touched.

## 8. Sources (all read 2026-10-20, kept)

Cognito: developer guide `https://docs.aws.amazon.com/cognito/latest/developerguide/` — [C1] `authentication-flows-public-server-side.html`, [C9] `amazon-cognito-user-pools-using-the-refresh-token.html`, [C10] `token-revocation.html`, [C11] `…-using-tokens-verifying-a-jwt.html`, [C12] `…-using-the-access-token.html`, [C13] `federation-endpoints.html`, [C14] `authorization-endpoint.html`, [C15] `token-endpoint.html`, [C16] `logout-endpoint.html`, [C17] `revocation-endpoint.html`, [C18] `quotas.html`, [C19] `authentication-flow-things-to-know.html`, [C20] `cognito-user-pool-managing-errors.html`, [C21] `user-pool-settings-client-apps.html`, [C22] `user-pool-settings-mfa.html`, [C25] `signing-up-users-in-your-app.html`, [C26] `authentication.html`; API reference `https://docs.aws.amazon.com/cognito-user-identity-pools/latest/APIReference/` — [C2] `API_InitiateAuth.html`, [C3] `API_RespondToAuthChallenge.html`, [C4] `API_SignUp.html`, [C5] `API_ConfirmSignUp.html`, [C6] `API_ForgotPassword.html`, [C7] `API_ConfirmForgotPassword.html`, [C8] `API_ResendConfirmationCode.html`, [C24] `API_CreateUserPoolClient.html`.
Keycloak: [K1] `https://www.keycloak.org/securing-apps/oidc-layers`, [K2] `https://www.keycloak.org/docs/latest/server_admin/index.html`, [K3] `https://www.keycloak.org/docs-api/latest/rest-api/index.html`, [K4] `…/docs/latest/upgrading/index.html`, [K6] GitHub releases, [K7] forum post (community).
Entra (`https://learn.microsoft.com/en-us/entra/`): [E1] `identity-platform/v2-oauth-ropc`, [E2] `…/reference-native-authentication-api`, [E3] `…/concept-native-authentication`, [E4] `https://devblogs.microsoft.com/identity/native-auth-for-external-id-ga/`, [E5] `…/access-tokens`, [E6] `…/access-token-claims-reference`, [E7] `…/id-tokens`, [E8] `…/security-tokens`, [E9] `…/v2-oauth2-auth-code-flow`, [E10] `…/refresh-tokens`, [E11] `…/v2-protocols-oidc`, [E12] `…/reply-url`, [E13] `…/configurable-token-lifetimes`, [E14] `identity/authentication/howto-password-smart-lockout`, [E15] `…/reference-error-codes`, [E16]-[E19] Microsoft Graph `user-post-users`, `user-update`, `authenticationmethod-resetpassword`, `user-revokesigninsessions`.
reCAPTCHA: [R1] `https://docs.cloud.google.com/recaptcha/docs/verify`, [R2] `https://developers.google.com/recaptcha/docs/v3`, [R3] `…/docs/verify`, [R4] `https://docs.cloud.google.com/recaptcha/docs/migration-overview`, [R5] `…/faq`, [R6] `…/create-assessment-website`, [R7] `…/interpret-assessment-website`.

## 9. Risks & mitigations

- **R1 — Same-site requirement on the web.** A `SameSite=Strict` cookie is sent only when the page and the backend are the **same site** (for example `app.example.com` and `api.example.com`). A backend on an unrelated domain cannot refresh by cookie. → Check warns when the configured page origin and the backend address are cross-site, and the KB states the rule. Without a refresh cookie the session still works until the access token expires, then `onSignedOut` follows (R5's failure path).
- **R2 — `Secure` cookies on `http://localhost`.** Chrome and Firefox treat localhost as a secure context. Safari's behaviour for `Secure` cookies on plain-http localhost is **UNVERIFIED**. → The dev server serves TLS where `tlsutil.rs` provides it; the browser test records which browsers ran.
- **R3 — The HTTP layer may change under the routes.** WASM-003's plan moves to a Leptos/axum router (its C3, §2.2), while `cobolt-server/tests/dependency_guards.rs` asserts that `hyper` is absent. That conflict is WASM-003's to settle. → The handlers are transport-neutral (D5), so either outcome is a mount change, not a rewrite.
- **R4 — The WebSocket envelope is not aligned.** `net.rs` sends `{"type":"auth"…}` and `{"type":"call"…}` (ABI A8, "the player's proposal"), while the server parses `{"t":"auth"…}` after a `hello` (`proto.rs` `parse_client`). Identity no longer depends on it (C2), but AC2's "call an exported program with the resulting token" does. → WASM-014/016's alignment comes first; AC2 records it as a precondition.
- **R5 — Multi-instance backends.** The hosted handle and the reCAPTCHA replay cache are per instance. → On the web, `hosted/finish` falls back to the cookie (D5). On mobile, a hosted sign-in that lands on another instance fails cleanly and the user retries. The replay cache's per-instance gap is stated. Shared state is WASM-019's (deployment).
- **R6 — The provider facts are documentation, not a live run.** → Every **UNVERIFIED** fact is configuration. Cognito's `EMAIL_MFA` vs `EMAIL_OTP` naming is accepted both ways. Live runs against Cognito and Entra are the operator's (R12).
- **R7 — reCAPTCHA classic is being retired by Google.** → `CaptchaVerifier` is the seam for `CreateAssessment`, and the fail-open quota answer is a failure.
- **R8 — New dependencies.** `chacha20poly1305` (server) and `qrcode` (player) must pass the dependency guards (pure Rust, no C crypto, MSRV 1.92) and the licence check. → If `qrcode` fails a guard, the TOTP set-up screen shows the `otpauth://` link and the secret only. That is still R1-complete.
- **R9 — Drafts are uncompiled.** About 2,300 lines in `drafts/` were never built and predate C2 (`http.rs.draft` and `session.rs.draft` assume the WebSocket and session-handle design). → They are input, not truth: each file is rebuilt against this plan with its tests, and the session-handle draft is dropped.
- **R10 — GUI guardrail.** The Designer paint of the new control uses the common control frame. → If it adds border geometry, the GUI Border Validation Agent is called at `/implement`. DataGrid is not affected.

## 10. Test strategy

Each test reports quantified, human-readable results. No test asserts a measurement its run did not produce.

| AC | Test | Where | Reports |
|---|---|---|---|
| AC1 | each screen, the Show flags honoured, six-language defaults per `Locale`, the new screens (MFA kinds, NewPassword, MfaSetup, Choose), the consent hold/grant/deny | `cobolt-web-player` `auth.rs`, `view/auth.rs` tests (extending the 29 that exist) | screens × languages checked |
| AC1 | accessibility: in Chrome through the `cobolt-web-bundle` harness, the **accessibility tree** (CDP `Accessibility.getFullAXTree`) of every screen is checked by Rust rules — every input has a name, one live region, the submit is reachable by keyboard, no focus trap. This is the "axe-style" check, done in Rust because no JavaScript library is vendored (PRIME DIRECTIVE) | `cobolt-web-bundle/tests` | rules × screens, failures by node |
| AC2 | create, confirm, sign in, call an exported program with the token (accepted), sign out; forgot and reset; MFA when the realm requires it | against Keycloak in a container; **runs only with `PR_KEYCLOAK_URL` set**, and skips with a printed reason otherwise | which steps ran |
| AC3 | unknown user vs wrong password: identical body; timing medians over 50 attempts each, difference under 50 ms against the mock; reset for an unknown vs a known address identical | `cobolt-server/tests/identity_enumeration.rs` | both medians and the difference |
| AC4 | reCAPTCHA: no token, replayed token, another action, another hostname, score below the threshold, "Over free quota" — each refused with the generic message; a good one accepted | `cobolt-server` identity tests with a mocked `siteverify` | six cases pass/fail |
| AC5 | in Chrome after sign-in, `localStorage` and `sessionStorage` hold no token (the test reads them) and `document.cookie` does not show `__Host-pr_refresh`; a forced expiry gives exactly one `POST /auth/refresh` and the call proceeds | `cobolt-web-bundle` harness + the mock backend | storage contents, request count |
| AC6 | the exact-value scan finds a planted client secret, reCAPTCHA secret and cookie key in a form, a copybook and the bundle; the same values in `backend.toml` as `env:` are accepted; `PasswordLabel` is not flagged | `cobolt-web-bundle/tests`, `cobolt-compiler` build test | hits by file and setting name |
| AC7 | a full flow (in-app and hosted) with the backend log and the browser console captured; neither contains any token string or password issued in the run | `cobolt-server/tests/loopback.rs`, extending `the_log_has_neither_the_token_nor_a_field_…`, + harness console | strings searched, hits |
| AC8 | **one conformance suite**, parametrised over the Cognito, Keycloak and Entra (workforce and External ID native) mocks (`idp_mock.rs` speaking each product's documented wire shapes): every operation the capability table marks `Y` succeeds, and every `-`/hosted operation answers "not supported here" with the hosted fallback; plus a test that the table in §6 and `capabilities()` agree | `cobolt-server/tests/identity_conformance.rs` | operations × adapters matrix |
| AC9 | `.cfrm` round-trip of an Authentication control (incl. a legacy `unknown:Authentication` file) and of `requires-sign-in`; `ControlType::ALL`-driven tests; `prebuilt_chunked_kb_matches_the_published_documentation` | `cobolt-forms`, `cobolt-ide`, `cobolt-compiler` | — |
| R5, D5–D7 | cookie attributes; a tampered or old cookie refused; preflight required (no `X-PR-Identity` → 403); an origin not in the list refused; `return` to a foreign origin refused; hosted `state` mismatch refused; handle single-use and 60 s | `cobolt-server` identity route tests | case list |
| R11 | a protected form opened signed out gives the sign-in form, then the requested form with its parameters; sign-out closes it; Check without a sign-in form fails | player host tests, `cobolt-compiler` check test | — |
| D15 | `AUTH-OP` outside a backend gives the same error in `rcrun run-form` and the compiled binary; refused by the browser generator's Check | `cobolt-runtime`, `cobolt-wasm` tests | — |
| D4 | the generated programs parse, pass `cobolt-semantic`, are deterministic, and carry the banner; `Backend::load` merges the fragment and refuses a duplicate export | `cobolt-codegen`, `cobolt-server` tests | — |

**Manual checks:** the Designer shows the control in the toolbox of a Web project and not of a Desktop project; the Identity settings page in six languages (explicit sizes, no self-inflation); Test connection against the mocks. The PowerRustCOBOL MCP tools (`render_form`, `check`) are used, not UI automation; the operator judges the look.

**Live runs are the operator's** (R12): Cognito, Entra workforce and Entra External ID. Each needs a pool or tenant this machine does not have. The acceptance records which ran.

## 11. Steering compliance

- [ ] **i18n:** IDE strings as `Tr` ×6 (D12). The control's end-user defaults ×6 in `cobolt-forms/src/auth_texts.rs`, overridable by the developer.
- [ ] **Generated code:** the `AUTH-*` programs, copybook and exports fragment carry the banner, are regenerated on Build / Run / Check and are never hand-edited (D4). Their COBOL follows the extensions ruling (one inline `COBOL::"AUTH-OP"`, no scratch items).
- [ ] **Interpreter/binary parity:** D15.
- [ ] **System KB:** doc tables + regenerated `assets/knowledge/chunked.data` in the same change as the vocabulary (§7).
- [ ] **Credentials:** secrets only as `env:`/`file:` (D13); exact-value scan (D14); fixtures use generated values; the example project's `backend.toml` holds references only.
- [ ] **PRIME DIRECTIVE:** no JavaScript added or changed (the hosted return reuses `loader.js`'s `pr-code`/`pr-state`; the accessibility check is Rust over CDP); the drafts are Rust.
- [ ] **User code is sacred:** generation writes only `backend/generated/identity/`; `exports.toml` and `backend/src/` are the developer's.
- [ ] **Docs:** `README.md` and `docs/` untouched on 2.x.x; what changes about the ABI is an amendment row in `specs/WASM-ABI-v1.md`.
- [ ] **Versioning:** feature on `features-2x`; every commit bumps `z` in `crates/cobolt-ide/src/version.rs` with a `CHANGELOG.md` entry; never `x`/`y`.
- [ ] **No "cobolt" in user-facing text;** COBOL identifiers English (`AUTH-REQUEST`, `ONE-TIME-CODE`).

## 12. Open questions (not decided here)

- **Q1 (residual, spec) — the default flow per adapter.** Working assumption, as the spec recommends: Cognito in-app; Keycloak hosted; Entra workforce hosted; Entra External ID hosted with `native` as an opt-in. *To confirm by the operator.* The "verify each product's capability" part of Q1 is done from documentation (§3); live verification is the operator's (R12).
- **Keycloak container (R12, AC2).** Pulling a Keycloak image is a download the operator has not authorised; Docker's daemon was not running on 2026-10-20. Until then AC2 runs only where `PR_KEYCLOAK_URL` is provided.
- **Biometric unlock (Q7 / WASM-012 Q1).** Q7 is decided ("confirmed"), but WASM-012 Q1 still says whether biometric unlock enters 2.0 is *to confirm*. Working assumption: it does, as the secure store's user-presence condition (D6). If WASM-012 leaves it out, D6 stores without it.
- **Cross-site backends (§9 R1).** Is a backend on a different site from the page a supported deployment? Working assumption: no for cookie refresh (Check warns); the session then lasts one access-token lifetime.
- **reCAPTCHA Enterprise.** If the operator wants `CreateAssessment`, it needs a Google Cloud project and an API key the operator creates; the seam is `CaptchaVerifier`.
- **SRP for Cognito.** Out of this cut: a Cognito client that forbids `USER_PASSWORD_AUTH` is reported by Test connection, with the instruction to allow it or use the hosted flow.
- **Envelope alignment (§9 R4)** and **the HTTP layer (§9 R3)** belong to WASM-014/016 and WASM-003; this plan depends on their outcome and decides neither.
