# 🎇 Sparkler

A simple [Retrospring](https://github.com/Retrospring/retrospring)-inspired service.

## Usage

For Sparkler to properly serve static assets, you must link the `static` directory to `~/.config/xsu-apps/sparkler/static`:

```bash
ln -s ~/storage/code/xsu/crates/sparkler/static ~/.config/xsu-apps/sparkler/static
```

## Authentication

Sparkler requires a [`xsu-authman`](https://github.com/hkauso/xsu) connection to provide authentication.
