# 🌼 Lily

Lily is a simple version control system using SQLite.

## Structure

Lily is designed to be a simple and easily readable. Lily creates a `.garden` directory in the working tree (where the files are) of a project after running initializing a project in a directory.

This "garden" directory contains a few things:

* `info` - A TOML file which details information about the garden (default branch, local current branch, etc)
* `lily.db` - An SQLite database containing branches and commits
* `tracker.db` - An SQLite database containing repository issues (not cloned by default, this can be done with `ly tracker`)
* `objects` - A directory containing files (named with an ID that matches in the `lily.db` file) which are in the tar.gzip format; they represent the entire working tree at the state of a commit

### Patch Format

Changes to files are stored in a very easy to read format:

```rust
pub enum ChangeMode {
    Added,
    Deleted,
}

pub type Change = (i32, ChangeMode, String);

pub struct Patch {
    pub files: HashMap<String, Vec<Change>>,
}
```
