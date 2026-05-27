# vue-template-compiler NAPI alias

Compatibility package that exposes the locked Vue 2.6 and Vue 2.7 `vue-template-compiler` package name through the Rust-backed `@vuec-rs/native` bridge.

This package is used by conformance and package-alias verification. Coverage reports still distinguish `rust-backed`, `mixed`, and `shim-backed` behavior.
