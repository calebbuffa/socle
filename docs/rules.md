# "Power of Ten" Rules — Adapted for Rust

These guidelines reinterpret NASA’s safety-critical coding rules in idiomatic Rust.

---

## 1. Prefer simple, explicit control flow

- Avoid overly complex branching and deeply nested logic.
- Prefer `if`, `match`, `while`, `for`.
- Avoid clever or implicit control flow (e.g., chaining that obscures intent).

---

## 2. Ensure loops have clear, bounded behavior

- Prefer iterators and ranges (`0..n`) when possible.
- Avoid unbounded loops unless they are clearly controlled (`loop {}` with explicit break conditions).
- Make termination conditions obvious and reviewable.

---

## 3. Avoid runtime allocation in critical paths

- Prefer stack allocation and fixed-size data structures.
- Pre-allocate (`Vec::with_capacity`) when needed.
- Avoid unnecessary cloning or heap allocation in tight or real-time code.

---

## 4. Keep functions small and focused

- Aim for functions that are easy to read in one view (~20–60 lines).
- Each function should do one thing.
- Extract helper functions instead of growing complexity.

---

## 5. Encode invariants in types and assertions

- Use Rust’s type system (`enum`, `struct`, `Option`, `Result`) to enforce correctness.
- Add `debug_assert!` / `assert!` for critical invariants.
- Prefer compile-time guarantees over runtime checks when possible.

---

## 6. Minimize scope and mutability

- Declare variables in the smallest possible scope.
- Prefer immutability (`let` over `let mut`).
- Avoid global mutable state (`static mut`).

---

## 7. Handle all errors explicitly

- Do not ignore `Result` or `Option`.
- Use `?` to propagate errors cleanly.
- Avoid `unwrap()` / `expect()` in production or critical code.

---

## 8. Avoid macros unless necessary

- Prefer functions, traits, and generics over macros.
- Use macros only when they clearly improve clarity or eliminate repetition.
- Avoid complex or opaque macro logic.

---

## 9. Limit unsafe code and complex indirection

- Avoid `unsafe` unless absolutely required.
- Encapsulate unsafe code in small, well-reviewed modules.
- Prefer references and ownership over raw pointers.

---

## 10. Enforce strict compilation and analysis

- Compile with warnings as errors (`-D warnings`).
- Use `clippy` and fix all lints.
- Run `rustfmt` for consistent formatting.
- Use static analysis and thorough code review.
