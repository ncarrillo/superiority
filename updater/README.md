# Superiority updater

Cross-platform updater for the Superiority desktop app.

It downloads the selected release, verifies it, installs it, and relaunches the
app.

```sh
cargo test --release -p superiority-updater
```

Release commands are listed in [`scripts/README.md`](../scripts/README.md).
