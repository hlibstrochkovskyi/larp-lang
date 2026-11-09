# Third-party Rust dependencies

LARP source code is licensed under the MIT license in [LICENSE](LICENSE).
The `vendor/` directory contains unchanged upstream Rust packages with their
original source, license headers and included license notices. Those packages
retain their upstream licenses and authorship.

## Selected dependencies

The CLI uses the `toml` parser with `default-features = false` and the `std` and
`parse` features. The selected build compiles five external crates: `toml`,
`toml_datetime`, `toml_parser`, `serde_spanned`, and `winnow`. It does not enable
Serde serialization, TOML writing, ordered maps, debug output, or hash features.

Cargo also locks optional packages referenced by those crates' feature metadata.
All 15 external packages in the main `Cargo.lock` are vendored. The existing
isolated fuzz workspace has 47 external locked packages; the shared directory
source contains the union of 61 exact package versions. This preserves that
workspace's dependency resolution when it inherits the root Cargo configuration.
No fuzz target, seed, campaign setting, or dependency version was changed to
create this snapshot. A locked or vendored package does not imply that its code
is compiled into or linked with the CLI. Fuzz instrumentation remains separate.

## Provenance and licenses

Each archive was downloaded from `https://static.crates.io/crates/NAME/NAME-VERSION.crate`.
Its SHA-256 was checked against the corresponding version record in the official
[crates.io registry index](https://index.crates.io/config.json) and the relevant
lockfile. Cargo generated this directory with
`cargo vendor --locked --offline --sync fuzz/Cargo.toml`. Generated
`.cargo-checksum.json` files cover all vendored file contents. Existing main
workspace vendor directories were compared byte-for-byte with the combined
snapshot and were unchanged.

The table records upstream package license metadata. The `main`/`fuzz` column
identifies lockfile membership, not linkage. Included notices remain inside each
package, and upstream source headers are retained. Some published target-only
packages omit standalone license texts; companion upstream notices fetched at
the source commit recorded in their `.cargo_vcs_info.json` are listed below.

| Package | Version | Upstream license | Lockfiles | Archive SHA-256 |
| --- | --- | --- | --- | --- |
| [anyhow](vendor/anyhow/Cargo.toml) | `1.0.102` | MIT OR Apache-2.0 | fuzz | `7f202df86484c868dbad7eaa557ef785d5c66295e41b460ef922eca0723b842c` |
| [arbitrary](vendor/arbitrary/Cargo.toml) | `1.5.0` | MIT OR Apache-2.0 | fuzz | `3bc62ac97cc33321f50863d514c3bc38a453947a8f9e781137e47c7401020aed` |
| [bitflags](vendor/bitflags/Cargo.toml) | `2.11.1` | MIT OR Apache-2.0 | fuzz | `c4512299f36f043ab09a583e57bceb5a5aab7a73db1805848e8fef3c9e8c78b3` |
| [cc](vendor/cc/Cargo.toml) | `1.8.0` | MIT OR Apache-2.0 | fuzz | `6651c9ed80effdc7db0ff72512157f901af5e3549e341e24b1dd4887d836d838` |
| [cfg-if](vendor/cfg-if/Cargo.toml) | `1.0.4` | MIT OR Apache-2.0 | fuzz | `9330f8b2ff13f34540b44e946ef35111825727b38d33286ef986142615121801` |
| [equivalent](vendor/equivalent-1.0.2/Cargo.toml) | `1.0.2` | Apache-2.0 OR MIT | fuzz | `877a4ace8713b0bcf2a4e7eec82529c029f1d0619886d18145fea96c3ffe5c0f` |
| [equivalent](vendor/equivalent/Cargo.toml) | `1.0.3` | Apache-2.0 OR MIT | main | `00d174d5400e5e8fd687ad1049e2f578285fa914201b1af7e8b112a4546bd826` |
| [find-msvc-tools](vendor/find-msvc-tools/Cargo.toml) | `0.1.14` | MIT OR Apache-2.0 | fuzz | `aedcfb3409746eddb02b9e19ebda1c3394f759a152e48ee875a0844d1b955484` |
| [foldhash](vendor/foldhash/Cargo.toml) | `0.1.5` | Zlib | fuzz | `d9c4f5dac5e15c24eb999c26181a6ca40b39fe946cbe4c263c7209467bc83af2` |
| [getrandom](vendor/getrandom/Cargo.toml) | `0.4.2` | MIT OR Apache-2.0 | fuzz | `0de51e6874e94e7bf76d726fc5d13ba782deca734ff60d5bb2fb2607c7406555` |
| [hashbrown](vendor/hashbrown-0.15.5/Cargo.toml) | `0.15.5` | MIT OR Apache-2.0 | fuzz | `9229cfe53dfd69f0609a49f65461bd93001ea1ef889cd5529dd176593f5338a1` |
| [hashbrown](vendor/hashbrown/Cargo.toml) | `0.17.1` | MIT OR Apache-2.0 | main + fuzz | `ed5909b6e89a2db4456e54cd5f673791d7eca6732202bbf2a9cc504fe2f9b84a` |
| [heck](vendor/heck/Cargo.toml) | `0.5.0` | MIT OR Apache-2.0 | fuzz | `2304e00983f87ffb38b55b444b5e3b60a884b5d30c0fca7d82fe33449bbe55ea` |
| [id-arena](vendor/id-arena/Cargo.toml) | `2.3.0` | MIT/Apache-2.0 | fuzz | `3d3067d79b975e8844ca9eb072e16b31c3c1c36928edf9c6789548c524d0d954` |
| [indexmap](vendor/indexmap-2.14.0/Cargo.toml) | `2.14.0` | Apache-2.0 OR MIT | fuzz | `d466e9454f08e4a911e14806c24e16fba1b4c121d1ea474396f396069cf949d9` |
| [indexmap](vendor/indexmap/Cargo.toml) | `2.14.2` | Apache-2.0 OR MIT | main | `cc4e190f5d26ca7051642629da2c52fc03bde85a03197c99408dcd291734c855` |
| [itoa](vendor/itoa/Cargo.toml) | `1.0.18` | MIT OR Apache-2.0 | fuzz | `8f42a60cbdf9a97f5d2305f08a87dc4e09308d1276d28c869c684d7777685682` |
| [jobserver](vendor/jobserver/Cargo.toml) | `0.1.35` | MIT OR Apache-2.0 | fuzz | `1c00acbd29eabad4a2392fa0e921c874934dbbf4194312ad20f04a0ed67a3cb3` |
| [leb128fmt](vendor/leb128fmt/Cargo.toml) | `0.1.0` | MIT OR Apache-2.0 | fuzz | `09edd9e8b54e49e587e4f6295a7d29c3ea94d469cb40ab8ca70b288248a81db2` |
| [libc](vendor/libc/Cargo.toml) | `0.2.186` | MIT OR Apache-2.0 | fuzz | `68ab91017fe16c622486840e4c83c9a37afeff978bd239b5293d61ece587de66` |
| [libfuzzer-sys](vendor/libfuzzer-sys/Cargo.toml) | `0.4.13` | (MIT OR Apache-2.0) AND NCSA | fuzz | `a9fd2f41a1cba099f79a0b6b6c35656cf7c03351a7bae8ff0f28f25270f929d2` |
| [log](vendor/log/Cargo.toml) | `0.4.29` | MIT OR Apache-2.0 | fuzz | `5e5032e24019045c762d3c0f28f5b6b8bbf38563a65908389bf7978758920897` |
| [memchr](vendor/memchr/Cargo.toml) | `2.8.0` | Unlicense OR MIT | fuzz | `f8ca58f447f06ed17d5fc4043ce1b10dd205e060fb3ce5b979b8ed8e59ff3f79` |
| [prettyplease](vendor/prettyplease/Cargo.toml) | `0.2.37` | MIT OR Apache-2.0 | fuzz | `479ca8adacdd7ce8f1fb39ce9ecccbfe93a3f1344b3d0d97f20bc0196208f62b` |
| [proc-macro2](vendor/proc-macro2-1.0.106/Cargo.toml) | `1.0.106` | MIT OR Apache-2.0 | fuzz | `8fd00f0bb2e90d81d1044c2b32617f68fcb9fa3bb7640c23e9c748e53fb30934` |
| [proc-macro2](vendor/proc-macro2/Cargo.toml) | `1.0.107` | MIT OR Apache-2.0 | main | `985e7ec9bb745e6ce6535b544d84d6cd6f7ad8bd711c398938ae983b91a766d9` |
| [quote](vendor/quote-1.0.45/Cargo.toml) | `1.0.45` | MIT OR Apache-2.0 | fuzz | `41f2619966050689382d2b44f664f4bc593e129785a36d6ee376ddf37259b924` |
| [quote](vendor/quote/Cargo.toml) | `1.0.47` | MIT OR Apache-2.0 | main | `1fbf4db142a473a8d80c26bbf18454ed458bf8d26c8219c331daecfdbd079001` |
| [r-efi](vendor/r-efi/Cargo.toml) | `6.0.0` | MIT OR Apache-2.0 OR LGPL-2.1-or-later | fuzz | `f8dcc9c7d52a811697d2151c701e0d08956f92b0e24136cf4cf27b57a6a0d9bf` |
| [semver](vendor/semver/Cargo.toml) | `1.0.28` | MIT OR Apache-2.0 | fuzz | `8a7852d02fc848982e0c167ef163aaff9cd91dc640ba85e263cb1ce46fae51cd` |
| [serde](vendor/serde/Cargo.toml) | `1.0.228` | MIT OR Apache-2.0 | fuzz | `9a8e94ea7f378bd32cbbd37198a4a91436180c5bb472411e48b5ec2e2124ae9e` |
| [serde_core](vendor/serde_core-1.0.228/Cargo.toml) | `1.0.228` | MIT OR Apache-2.0 | fuzz | `41d385c7d4ca58e59fc732af25c3983b67ac852c1a25000afe1175de458b67ad` |
| [serde_core](vendor/serde_core/Cargo.toml) | `1.0.229` | MIT OR Apache-2.0 | main | `67dca2c9c51e58a4791a4b1ed58308b39c64224d349a935ab5039aa360942a48` |
| [serde_derive](vendor/serde_derive-1.0.228/Cargo.toml) | `1.0.228` | MIT OR Apache-2.0 | fuzz | `d540f220d3187173da220f885ab66608367b6574e925011a9353e4badda91d79` |
| [serde_derive](vendor/serde_derive/Cargo.toml) | `1.0.229` | MIT OR Apache-2.0 | main | `e7a5d71263a5a7d47b41f6b3f06ba276f10cc18b0931f1799f710578e2309348` |
| [serde_json](vendor/serde_json/Cargo.toml) | `1.0.150` | MIT OR Apache-2.0 | fuzz | `e8014e44b4736ed0538adeecded0fce2a272f22dc9578a7eb6b2d9993c74cfb9` |
| [serde_spanned](vendor/serde_spanned/Cargo.toml) | `1.1.2` | MIT OR Apache-2.0 | main | `4e7523beb55eece201a2356bee0bbca0d1ab466c14c07703b2e0ee6d42cb0c2c` |
| [shlex](vendor/shlex/Cargo.toml) | `2.0.1` | MIT OR Apache-2.0 | fuzz | `f8fadd59c855ef2080decdef8ff161eb6661b86933c9d82e5ba29dc602a55aba` |
| [syn](vendor/syn-2.0.117/Cargo.toml) | `2.0.117` | MIT OR Apache-2.0 | fuzz | `e665b8803e7b1d2a727f4023456bbbbe74da67099c585258af0ad9c5013b9b99` |
| [syn](vendor/syn/Cargo.toml) | `3.0.9` | MIT OR Apache-2.0 | main | `d78c8dee4c7bf0e14673097256fed6142ce9d3b85a408189d07482442145823b` |
| [toml](vendor/toml/Cargo.toml) | `1.1.8+spec-1.1.0` | MIT OR Apache-2.0 | main | `20489e00e4d8741d6be680764cc12e270655e375a20d1011e844a9c3379e678d` |
| [toml_datetime](vendor/toml_datetime/Cargo.toml) | `1.1.2+spec-1.1.0` | MIT OR Apache-2.0 | main | `2b86d767906c6c42421dcba507eb9d203e779497710a47782a224bb871653053` |
| [toml_parser](vendor/toml_parser/Cargo.toml) | `1.1.5+spec-1.1.0` | MIT OR Apache-2.0 | main | `baa693a8032d7e1cada7d0041e96126df243179ff061456783ac7f12bda4744c` |
| [toml_writer](vendor/toml_writer/Cargo.toml) | `1.1.3+spec-1.1.0` | MIT OR Apache-2.0 | main | `06bdbd8cfc056b8d2e2e85f29b56a3bdbecb527cef81eb39e3e7b98af4652770` |
| [unicode-ident](vendor/unicode-ident-1.0.24/Cargo.toml) | `1.0.24` | (MIT OR Apache-2.0) AND Unicode-3.0 | fuzz | `e6e4313cd5fcd3dad5cafa179702e2b244f760991f45397d14d4ebf38247da75` |
| [unicode-ident](vendor/unicode-ident/Cargo.toml) | `1.0.27` | (MIT OR Apache-2.0) AND Unicode-3.0 | main | `a2c754d6c33795a1c324727428e5a7dedb5b06195f9890bdbcba760d3e246563` |
| [unicode-xid](vendor/unicode-xid/Cargo.toml) | `0.2.6` | MIT OR Apache-2.0 | fuzz | `ebc1c04c71510c7f702b52b7c350734c9ff1295c464a03335b00bb84fc54f853` |
| [wasip2](vendor/wasip2/Cargo.toml) | `1.0.3+wasi-0.2.9` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | fuzz | `20064672db26d7cdc89c7798c48a0fdfac8213434a1186e5ef29fd560ae223d6` |
| [wasip3](vendor/wasip3/Cargo.toml) | `0.4.0+wasi-0.3.0-rc-2026-01-06` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | fuzz | `5428f8bf88ea5ddc08faddef2ac4a67e390b88186c703ce6dbd955e1c145aca5` |
| [wasm-encoder](vendor/wasm-encoder/Cargo.toml) | `0.244.0` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | fuzz | `990065f2fe63003fe337b932cfb5e3b80e0b4d0f5ff650e6985b1048f62c8319` |
| [wasm-metadata](vendor/wasm-metadata/Cargo.toml) | `0.244.0` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | fuzz | `bb0e353e6a2fbdc176932bbaab493762eb1255a7900fe0fea1a2f96c296cc909` |
| [wasmparser](vendor/wasmparser/Cargo.toml) | `0.244.0` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | fuzz | `47b807c72e1bac69382b3a6fb3dbe8ea4c0ed87ff5629b8685ae6b9a611028fe` |
| [winnow](vendor/winnow/Cargo.toml) | `1.0.4` | MIT | main | `23b97319f7b8343df12cc98938e5c3eb436064524c8d2b4e30a1d3a36eecdf81` |
| [wit-bindgen](vendor/wit-bindgen-0.51.0/Cargo.toml) | `0.51.0` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | fuzz | `d7249219f66ced02969388cf2bb044a09756a083d0fab1e566056b04d9fbcaa5` |
| [wit-bindgen](vendor/wit-bindgen/Cargo.toml) | `0.57.1` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | fuzz | `1ebf944e87a7c253233ad6766e082e3cd714b5d03812acc24c318f549614536e` |
| [wit-bindgen-core](vendor/wit-bindgen-core/Cargo.toml) | `0.51.0` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | fuzz | `ea61de684c3ea68cb082b7a88508a8b27fcc8b797d738bfc99a82facf1d752dc` |
| [wit-bindgen-rust](vendor/wit-bindgen-rust/Cargo.toml) | `0.51.0` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | fuzz | `b7c566e0f4b284dd6561c786d9cb0142da491f46a9fbed79ea69cdad5db17f21` |
| [wit-bindgen-rust-macro](vendor/wit-bindgen-rust-macro/Cargo.toml) | `0.51.0` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | fuzz | `0c0f9bfd77e6a48eccf51359e3ae77140a7f50b1e2ebfe62422d8afdaffab17a` |
| [wit-component](vendor/wit-component/Cargo.toml) | `0.244.0` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | fuzz | `9d66ea20e9553b30172b5e831994e35fbde2d165325bec84fc43dbf6f4eb9cb2` |
| [wit-parser](vendor/wit-parser/Cargo.toml) | `0.244.0` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | fuzz | `ecc8ac4bc1dc3381b7f59c34f00b67e18f910c2c0f50015669dde7def656a736` |
| [zmij](vendor/zmij/Cargo.toml) | `1.0.21` | MIT | fuzz | `b8848ee67ecc8aedbaf3e4122217aff892639231befc6a1b58d29fff4c2cabaa` |

## Companion upstream notices

These files supplement published crates whose archives omit repository-level
notices; they do not alter the vendored packages:

- [r-efi AUTHORS](vendor-notices/r-efi/AUTHORS), from
  [r-efi commit 7e1b032](https://github.com/r-efi/r-efi/blob/7e1b0322d31d625f81a5656096330934f9cd835d/AUTHORS).
- [wasm-tools MIT](vendor-notices/wasm-tools/LICENSE-MIT),
  [Apache](vendor-notices/wasm-tools/LICENSE-APACHE), and
  [Apache with LLVM exception](vendor-notices/wasm-tools/LICENSE-Apache-2.0_WITH_LLVM-exception),
  from [wasm-tools commit d4e317f](https://github.com/bytecodealliance/wasm-tools/tree/d4e317f22c3bace76cb3205003bcc34b4929037d).
  This covers the matching wasm-encoder, wasm-metadata, wasmparser, wit-component,
  and wit-parser repository snapshot.
- [wasi-rs MIT](vendor-notices/wasi-rs/LICENSE-MIT),
  [Apache](vendor-notices/wasi-rs/LICENSE-APACHE), and
  [Apache with LLVM exception](vendor-notices/wasi-rs/LICENSE-Apache-2.0_WITH_LLVM-exception),
  from [wasi-rs commit 06ce201](https://github.com/bytecodealliance/wasi-rs/tree/06ce201370fcde0d1b0d47cac8ecb1b0b312c9f9)
  for the wasip3 package.

Other special notices include the Unicode license retained by `unicode-ident`,
the Zlib notice in `foldhash/LICENSE`, and `memchr/UNLICENSE` alongside its MIT
license. `libfuzzer-sys` retains its Rust license files and embedded LLVM source
license headers; it is not linked into the LARP CLI. Keep all upstream and
companion notices in redistributed source packages.

## Updating the snapshot

Update dependency requirements deliberately, regenerate the appropriate lockfile
using the official registry, and stage a fresh snapshot in an ignored directory:

```sh
cargo vendor --locked --sync fuzz/Cargo.toml artifacts/vendor-update
```

By default, `cargo vendor` reads the original registry rather than the configured
replacement source. This update step requires network access and is separate
from ordinary builds. Verify registry checksums, review dependency and license
changes, replace `vendor/` with the reviewed snapshot, and update this inventory.
Keep the relative directory source configuration in `.cargo/config.toml`.

Validate both dependency graphs with fresh empty Cargo homes:

```sh
CARGO_HOME="$PWD/artifacts/vendor-check-home" \
CARGO_TARGET_DIR="$PWD/artifacts/vendor-check-target" \
cargo check --workspace --locked --offline
CARGO_HOME="$PWD/artifacts/fuzz-vendor-check-home" \
cargo metadata --manifest-path fuzz/Cargo.toml --locked --offline --format-version 1
```

Use fresh paths when repeating these checks. Metadata resolution does not execute
a fuzz campaign. Run the full checks in [CONTRIBUTING.md](CONTRIBUTING.md) before
committing. Building or running a prebuilt LARP CLI does not require fuzz tooling.
