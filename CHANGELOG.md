## Changelog

This file describes important user-facing changes in the `async-winit` crate.

## Version 0.2.1

- Fixes a compiler error.

## Version 0.2.0

- **Breaking:** Most types now include a `ThreadSafety` type parameter that controls whether or not it uses thread-safe (`Arc`, `Mutex`) or thread-unsafe (`Rc`, `RefCell`) primitives.
- This crate is now dual licensed under LGPL v3 and MPL 2.0, opposite to the previous AGPL v3 licensing.
