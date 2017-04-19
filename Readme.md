## Serde OSC

Serialization and deserialization of Open Sound Control 1.0 packets represented using structs/tuples/anything supported by serde.

Supports the 4 types specified in OSC 1.0: 'f', 'i', 's', 'b' corresponding to `f32`, `i32`, `String` and `Vec<u8>` ("blobs"), respectively, as well as nested OSC bundles.
Note that blobs must be wrapped in Serde's ByteBuf type.


## Usage

Refer to the examples under `examples/`. They can be run with (e.g.)

```sh
$ cargo --run to_from_vec
```

For more detailed usage (including using OSC bundles), refer to the `tests/`
directory and the documentation (below).


## Documentation

Documentation can be found over on [docs.rs](https://docs.rs/serde_osc/)


## License

Serde OSC tries to follow the same licensing as Serde. Serde OSC is licensed under either of

   * Apache License, Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0)
   * MIT license (http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Serde OSC by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
