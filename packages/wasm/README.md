# @vuec-rs/wasm

WASM package for the Rust Vue compiler.

Call `init()` before using compiler functions. In Node, the default loader imports `./pkg/vuec_wasm.js`; browsers may pass an alternate generated wasm-bindgen entry.

## Usage

```js
import { init, compileVue3Dom } from '@vuec-rs/wasm'

await init()
const result = compileVue3Dom('<div>{{ msg }}</div>', {
  mode: 'module',
  prefixIdentifiers: true
})

console.log(result.code)
```

## Verification

```bash
cargo xtask verify-wasm
cargo xtask verify-wasm-browser
cargo xtask verify-wasm-wasi
```
