# wish-tree

A small Rust library for describing directory structures and creating them as ZIP/tar archives or directly on the file
system.

## Usage

__Not yet published on cargo.io!__

Add this to your `Cargo.toml`:

```toml
[dependencies]
wish-tree = "0.1.0"
```

In your code:

```rust
use wish_tree::*;

// 1. Describe your desired target directory structure declaratively.
let my_dir = dir! {
    "dist" => dir! {
        "empty-dir" => dir!(),
        "index.js" => "build/index.js",
        "doc" => dir("build/doc").include("**/*.md"),
    },
    "README.txt" => text("This is a README file."),
};

// 2. "Render" it.
my_dir.render_to_fs("target/my-dir");
my_dir.render_to_zip("target/my-dir.zip");
my_dir.render_to_tar_gz("target/my-dir.tar.gz");
```

## Known issues

- Error handling has not been thought of so far. It just panics, e.g. if a file is missing. While panicking
  is okay for my use case (packaging a distribution bundle), it might not be for others. Also, it doesn't
  which file is missing.