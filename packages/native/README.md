# @vuec-rs/native

Node/NAPI package for the Rust Vue compiler.

The package loads a local `vuec_napi.node` binding when present, then falls back to the matching optional platform package such as `@vuec-rs/native-win32-x64`.

## Usage

```js
const vuec = require('@vuec-rs/native')

const result = vuec.compileVue3Dom('<div>{{ msg }}</div>', {
  mode: 'module',
  prefixIdentifiers: true
})

console.log(result.code)
```

## Verification

```bash
cargo xtask verify-napi
cargo xtask verify-napi-alias
cargo xtask verify-napi-api
cargo xtask verify-napi-platform
```

Official package-name aliases are verified separately from this loader package.
