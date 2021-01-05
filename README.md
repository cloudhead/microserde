Microserde
==========

Microserde is [miniserde](https://crates.io/crates/miniserde) minus the
dependencies.

All credit goes to David Tolnay for the original library.

From *miniserde*:

*Prototype of a data structure serialization library with several opposite
design goals from [Serde](https://serde.rs).*

Differences compared to `miniserde`:

* `ryu` crate is replaced with stdlib functionality
* `itoa` crate is replaced with stdlib functionality
* `serde` crate is removed from *dev-dependencies*

```toml
[dependencies]
microserde = "0.1"
```

### Example

```rust
use microserde::{json, Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
struct Example {
    code: u32,
    message: String,
}

fn main() -> microserde::Result<()> {
    let example = Example {
        code: 200,
        message: "reminiscent of Serde".to_owned(),
    };

    let j = json::to_string(&example);
    println!("{}", j);

    let out: Example = json::from_str(&j)?;
    println!("{:?}", out);

    Ok(())
}
```

### License

Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
