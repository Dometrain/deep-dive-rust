# Recording Guide -- Module 6: Authentication

Target runtime: **~82 minutes** of lesson content (two lessons, 6.1 and 6.2).

As with Modules 4 and 5, most of this audience is coming from **.NET/C#** --
every segment below has a ".NET parallel" line. One of them is a genuine
gotcha worth over-preparing for: a `from_fn` middleware that short-circuits
with `Err` does **not** behave like a handler returning
`Result<HttpResponse, AppError>` -- get segment 6.2.3 right, it's the highest
information-per-minute moment in this recording.

Two things should be open before you start recording:

- This folder (`deep-dive-module-6-authentication`) in the editor, with
  `src/password.rs`, `src/auth.rs`, `src/middleware.rs`, `src/routes/auth.rs`
  and this module's `README.md` in tabs.
- A terminal in this folder, plus
  `deep-dive-module-5-advanced-web-development` open in a second tab/window
  -- this module is a direct continuation, not a new app, same as every
  module has been since Module 2.

Pre-recording checklist:

- [ ] A C compiler is on `PATH` (`cc --version`) -- Module 3's FFI/`ETag`
      feature is still part of this app, so `build.rs` still needs one.
- [ ] `docker compose up -d db` run at least once (image pull is slow the
      first time) and confirmed healthy (`docker compose ps`) before you
      start `cargo run`.
- [ ] `cargo test --lib` passes (fast, no DB) -- also warms the C build
      cache.
- [ ] `cargo test` passes (needs Docker/OrbStack) -- confirms the whole
      login → token → protected-route flow works end to end, not just in
      isolation.
- [ ] `curl` ready for the applied-for-real demos. `POST /register` is your
      on-camera user seeding now -- one call creates `alice` *and* returns a
      token, with no raw Argon2 hash to paste. You register live in 6.2.5,
      which conveniently seeds the exact account the protected-route demo in
      6.2.6 then logs into.

---

## Cold open (~2 min)

Open by finishing Module 5's own sentence. That module closed with:
*"five modules in, and this app has gone from a single `main.rs` to
something with structured logging, a real database, a background worker,
request timeouts, CORS, and startup errors you can actually act on --
everything a real API needs except one thing: proving who's making the
request in the first place."* Land the actual thesis: *"today we close that
gap. Two, tightly connected halves: first, how to never store a password in
a way that matters if your database leaks -- that's `argon2`. Second, how to
prove a request is authenticated without a session store -- that's JWTs,
`jsonwebtoken`, and a middleware you've already seen the shape of. And
there's a third thing, smaller but worth its own segment: a mistake I
actually made building this module's own tests, about exactly how a
middleware is and isn't allowed to fail."*

---

## Lesson 6.1 -- JWT Fundamentals (~35 min)

### 1. JWT Basics (~4 min)
**File:** none -- README's curl walkthrough

- Draw the `xxxxx.yyyyy.zzzzz` shape on screen or in a scratch file. Read the
  wristband analogy from this module's README/`mod-6.md` almost verbatim --
  it lands better said slowly than summarized.
- State plainly: *"a JWT's payload is not encrypted, only encoded. Anyone
  who has the token can read it. What they can't do is forge one, or edit
  one, without the secret."*
- `.NET parallel`: *"`System.IdentityModel.Tokens.Jwt`'s `JwtSecurityToken`
  -- same three-part shape. Paste any token from either language into
  jwt.io and you'll read the same thing."*

### 2. Password Hashing with `argon2` (~8 min)
**File:** `src/password.rs`

- Run `cargo test password::` and read the four test names out loud before
  explaining any code: *"correct password verifies, wrong password doesn't,
  same password hashes differently twice, malformed hash doesn't panic.
  That last one matters as much as the first three."*
- Walk `hash_password`/`verify_password` function-by-function. Call out
  explicitly that this crate's API changed shape across versions --
  *"there's no `SaltString` or `OsRng` to manage by hand in the version
  this app pins; the salt is generated internally, and it's still unique
  every single call -- that's what the 'hashes differently twice' test
  proves."*
- Read the blender analogy from the README/`mod-6.md` -- smoothies,
  strawberries, salt as an extra ingredient.
- `.NET parallel`: *"`PasswordHasher<TUser>` in
  `Microsoft.AspNetCore.Identity` -- same job, PBKDF2 by default instead of
  Argon2. Argon2 is OWASP's current top pick for new work; PBKDF2 in
  ASP.NET Identity is mostly legacy compatibility, not a considered
  choice."*

### 3. Token Generation (~6 min)
**File:** `src/auth.rs` (`create_jwt`)

- Point at `ACCESS_TOKEN_TTL_MINUTES = 30` before anything else: *"thirty
  minutes, not the twenty-four hours a lot of tutorials default to -- see
  the exercises for making this configurable instead of hardcoded."*
- Walk `Claims`, then `encode(...).expect(...)`. Read the `.expect(...)`
  comment aloud: *"this app already has a pattern for 'this can't actually
  fail, here's why' -- `middleware::timing` does the same thing for a
  millisecond count. A plain string secret always encodes; there's no
  `Result` worth threading through for a failure mode that can't happen."*
- `.NET parallel`: *"`JwtSecurityTokenHandler().WriteToken(...)` over a
  `SecurityTokenDescriptor` -- same three inputs, same signed-string
  output."*

### 4. Token Validation (~6 min)
**File:** `src/auth.rs` (`validate_jwt`)

- Run `cargo test auth::` and land on
  `a_token_signed_with_a_different_secret_is_rejected` and
  `a_token_expires_after_its_ttl` specifically.
- Slow down on `Validation::new(Algorithm::HS256)`: *"this line pins the
  algorithm this server will accept. Some real JWT vulnerabilities in other
  ecosystems came from trusting whatever algorithm the token's own header
  claimed, instead of the server deciding up front. `jsonwebtoken` makes
  you name it explicitly -- that's not ceremony, that's the fix."*
- Mention the leeway gotcha from the test comment: *"the library gives you
  about sixty seconds of clock-skew tolerance by default on expiry checks
  -- worth knowing before you write a test asserting a token expired one
  minute ago is rejected and wonder why it isn't."*

### 5. Package Setup Gotcha, Said Out Loud (~3 min)
**File:** `Cargo.toml`

- This one bit while building the module -- say so directly: *"`cargo
  build` failed the first time I ran it against real JWTs, at test time, not
  compile time -- 'could not determine the process-level CryptoProvider.'
  `jsonwebtoken` 11 doesn't pick a crypto backend for you. This app pins
  `features = ["rust_crypto"]` -- pure Rust, no extra native dependency
  beyond the C compiler this app already needs for Module 3's FFI sample."*
- Land why this belongs in the recording: *"this is exactly the kind of
  thing that looks fine in a tutorial's code block and then breaks the
  first time you actually run it -- which is the whole reason this course
  runs everything before it goes in a README."*

---

## Lesson 6.2 -- Securing the API (~40 min)

### 1. Extending `AppError` (~5 min)
**File:** `error.rs`

- Open the diff mentally: two new variants (`Unauthorized` and `Conflict`) and
  their match arms. *"That's the whole change. This app has had one consistent
  error-to-HTTP pipeline since Module 1; this module doesn't add a second one,
  it just adds cases to the one that's already there."*
- Read the `Unauthorized` variant's doc comment: *"'Authentication failed,'
  not 'missing token' -- because the login handler, coming up, reuses this
  exact variant for a wrong password. A token-specific message would read
  strangely there."*
- Point at `Conflict` and tie it forward: *"this one's for the registration
  endpoint in 6.2.5 -- a taken username, mapped to `409`. Same idea as every
  variant here: it carries its own HTTP status, so the handler never touches a
  `StatusCode` itself."*

### 2. Config-Driven JWT Secret (~3 min)
**File:** `config.rs`, `config/default.toml`

- Quick: *"one field on `AppConfig`, one env-var override, exactly the
  pattern `database.url` already used. `config/default.toml`'s value is a
  placeholder that's obviously wrong in production -- same as
  `database.url` always was."*
- If time allows, a second, smaller gotcha worth one line: *"`main.rs`
  registers this as `web::Data::new(config.as_ref().clone())`, not
  `config.clone()` -- `config` is already an `Arc<AppConfig>`, and
  `web::Data` Arc-wraps internally too, so the naive version registers the
  wrong type. `cargo test` never caught this, because every test builds its
  `AppConfig` directly instead of going through `main.rs` -- it only showed
  up running the real server. See this module's README for the full story;
  worth a one-line mention here even if you don't do the full live repro."*

### 3. JWT Middleware -- and the Gotcha (~12 min -- the anchor segment)
**File:** `src/middleware.rs`

This is the one to slow down for, more than any single segment in Modules
1 through 5.

1. Show the "obvious," wrong-in-a-way-that-still-compiles version first, on
   screen, not run:
   ```rust
   let claims = extract_claims(&req, &secret).map_err(|_| AppError::Unauthorized)?;
   ```
   *"This compiles. It even works, against a real running server. It fails
   silently against this app's own test suite -- not a compiler error, a
   test panic with a confusing message."*
2. Run it (temporarily edit `require_auth` to this version, or have a
   branch/gist ready) against
   `middleware::tests::rejects_a_request_with_no_authorization_header` and
   show the panic: `test service call returned error: Unauthorized`.
3. Explain the actual mechanism, reading from this module's README's "hard-
   won lesson" section: *"every other error in this app crosses a
   `Result<HttpResponse, AppError>` boundary at a handler -- Actix's own
   `Responder` machinery resolves that internally, so the error never
   becomes a raw `Err` at the `Service::call` level at all. A `from_fn`
   middleware that short-circuits without calling `next` **is** that
   boundary. Only the real `HttpServer` dispatcher converts a top-level
   `Err` into a response -- I checked `actix-http`'s own dispatcher source
   to confirm this, it's not a guess. `test::call_service`, which every
   test in this app uses, skips that dispatcher on purpose, so it can
   catch exactly this class of bug -- and it did."*
4. Show the actual fix -- `req.into_response(response).map_into_boxed_body()`
   -- and explain `map_into_boxed_body()` on both branches: *"the function's
   return type is `impl MessageBody`, one concrete type either way Rust
   picks it. The success path and the rejection path produce different
   concrete body types unless you unify them -- this is the standard tool
   for that."*
5. Re-run the test, green. Land it: *"the lesson isn't `map_into_boxed_body`
   syntax. It's that a `from_fn` middleware bailing early is not the same
   situation as a handler returning `Result`, and the fix for one doesn't
   transfer to the other for free."*

### 4. User Model & Login Endpoint (~10 min)
**File:** `src/models/user.rs`, `src/routes/auth.rs`

- Point at `#[serde(skip_serializing)]` on `password_hash` first: *"this
  makes 'never leak the hash' a compile-time-adjacent guarantee, not a rule
  to remember at every call site."*
- Walk `authenticate_user`: *"reuses `password::verify_password` directly
  -- one implementation of 'how this app checks a password,' called from
  both the standalone test module and here."*
- The username-enumeration point, said plainly, twice if needed: *"a
  nonexistent username and a wrong password return the exact same
  `AppError::Unauthorized`, same status, same JSON body. `tests/auth_test.rs`
  has a test that asserts the two responses are byte-for-byte identical,
  not just 'both 401' -- that's the actual guarantee worth testing."*
- `.NET parallel`: *"`SignInManager<TUser>.PasswordSignInAsync`, minus the
  cookie ASP.NET Identity sets for you automatically -- this hands back a
  bearer token instead."*

### 5. Registration Endpoint -- the 409 that isn't a 500 (~7 min)
**File:** `src/routes/auth.rs` (`register`, `create_user`), `error.rs`

- Frame it against login: *"`/login` proves who you are; `/register` is how you
  get an account in the first place -- one handler, and it reuses the *exact*
  hashing `/login` verifies against (`password::hash_password`), never a second
  copy."*
- Show `register` returning `201 Created` with a token: *"registration logs you
  straight in -- the same `LoginResponse` as `/login`. `201`, not `200`, because
  a new resource -- the user -- just came into existence."*
- **The teaching beat -- the duplicate username:** *"two people pick the same
  name. Naively, the unique-constraint violation falls through the `#[from]
  sqlx::Error` conversion and becomes a `500` -- which tells the client 'the
  server broke, retry,' exactly the wrong thing. We match
  `db_error.is_unique_violation()` and remap to `AppError::Conflict` -- a `409`.
  One branch, and it's the difference between 'pick another name' and 'the
  server is down.'"* Show the `.map_err(...)` on the insert.
- Run the two new tests live:
  `registering_a_taken_username_is_rejected_with_conflict` and
  `a_registered_user_can_then_log_in`.
- Register `alice` on camera here (`curl -X POST /register ...`) -- the account
  and token you get carry straight into 6.2.6's protected-route demo.
- `.NET parallel`: *"`UserManager<TUser>.CreateAsync` -- its `IdentityResult`
  with a `DuplicateUserName` error is the same 'conflict, not crash' distinction
  ASP.NET Identity draws for you, instead of letting the DB throw."*

### 6. Protecting Routes -- Applied for Real (~5 min)
**File:** `src/routes/tasks.rs`

- Open the actual `.wrap(from_fn(crate::middleware::require_auth))` line
  inside `web::scope("/tasks")` -- not at the whole-`App` level in
  `main.rs`. *"That placement is the whole answer to 'which routes need a
  token': whichever scope this line sits inside, and nothing outside it.
  `/login` is registered separately, never wrapped."*
- Live demo:
  ```sh
  curl -i http://127.0.0.1:8080/tasks
  # 401
  curl -s -X POST http://127.0.0.1:8080/login -d '...' | jq -r .token
  curl -i http://127.0.0.1:8080/tasks -H "Authorization: Bearer <token>"
  # 200
  ```

### 7. Token Storage & Its Tradeoffs (~6 min)
**File:** `src/examples/module_6.rs` (`token_storage`)

- State this app's actual choice first: *"`Authorization` header,
  client-managed. Nothing in this app sets a cookie."*
- Walk the two-box analogy from the README: one storage option is
  see-through (XSS can read it), the other hands itself over automatically
  to anyone who asks nicely (CSRF) unless you also lock it to
  `same_site(Lax)`.
- Run `examples::module_6::token_storage`'s test, point at all three
  cookie attributes it asserts: `http_only`, `secure`, `same_site`.
- Name the stateless tension honestly, from the README: *"a refresh token
  done properly needs some server-side record to be revocable -- that's a
  deliberate, worthwhile break from 'JWTs are stateless,' not a
  contradiction of it."*

---

## Wrap-up (~3 min)

- Recap in one sentence each: `argon2` for passwords (never plaintext, never
  a bare `==`), `jsonwebtoken` for stateless-to-verify tokens, `from_fn` for
  auth middleware (same primitive as Module 5's timing header, not a new
  ceremony), `/register` and `/login` sharing one hashing implementation (with
  a taken username as a `409`, not a `500`), and the one genuinely new
  engineering lesson -- a `from_fn` middleware's early `Err` doesn't behave
  like a handler's `Result`.
- Point at the **Exercises** section of this module's `README.md` --
  explicitly say you're not solving them on camera (a configurable token TTL,
  per-reason auth logging, a `role` claim, a revocable refresh token, and the
  registration variants that build on the `/register` you just walked).
- Tease the next module with the same continuation framing used to open
  every module so far: *"six modules in, this app can prove who's making a
  request. It still can't tell you anything in real time -- every response
  so far has been request-in, response-out. Next module opens a connection
  that stays open."*

---

## Timing summary

| Segment | Target |
|---|---|
| Cold open | 2 min |
| 6.1.1 JWT Basics | 4 min |
| 6.1.2 Password Hashing with `argon2` | 8 min |
| 6.1.3 Token Generation | 6 min |
| 6.1.4 Token Validation | 6 min |
| 6.1.5 Package Setup Gotcha | 3 min |
| **Lesson 6.1 subtotal** | **~29 min** |
| 6.2.1 Extending `AppError` | 5 min |
| 6.2.2 Config-Driven JWT Secret | 3 min |
| 6.2.3 JWT Middleware -- the gotcha | 12 min |
| 6.2.4 User Model & Login Endpoint | 10 min |
| 6.2.5 Registration Endpoint -- the 409 that isn't a 500 | 7 min |
| 6.2.6 Protecting Routes | 5 min |
| 6.2.7 Token Storage & Tradeoffs | 6 min |
| **Lesson 6.2 subtotal** | **~48 min** |
| Wrap-up | 3 min |
| **Total** | **~82 min** |

If you're cutting to fit a tighter cap, cut 6.1.1 to a one-line callout
("wristband, not a lockbox -- see the README"), and cut 6.2.4's `.NET`
parallel if 6.1.2's already ran long. 6.2.5 can compress to just the 409-beat
(skip the auto-login framing) if you're tight -- but keep the 409-vs-500
distinction; it's the whole point of the segment. Don't cut 6.2.3 -- it's the
highest information-per-minute moment in this recording, the same role Module
5's middleware-ordering segment played, and it's a real bug this module's own
test suite caught, not a hypothetical.
