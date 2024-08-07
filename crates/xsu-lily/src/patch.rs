use std::collections::BTreeMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChangeMode {
    /// Something was added
    Added,
    /// Something was deleted
    Deleted,
}

/// A single change to a file
///
/// ```
/// (line number, mode, line)
/// ```
pub type Change = (usize, ChangeMode, String);

/// A file inside of a [`Patch`]
///
/// `(old content, changes)`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchFile(pub String, pub Vec<Change>);

impl Default for PatchFile {
    fn default() -> Self {
        Self(String::new(), Vec::new())
    }
}

impl PatchFile {
    /// Get a summary of the changes in this [`PatchFile`]
    ///
    /// # Returns
    /// `(total changes, additions, deletions)`
    pub fn summary(&self) -> (usize, usize, usize) {
        let mut additions = 0;
        let mut deletions = 0;

        for change in &self.1 {
            match change.1 {
                ChangeMode::Added => additions += 1,
                ChangeMode::Deleted => deletions += 1,
            }
        }

        (self.1.len(), additions, deletions)
    }

    /// Apply a [`PatchFile`] to a [`String`] and get the patched version of the [`String`]
    ///
    /// # Example
    /// ```rust
    /// let patch: PatchFile = ...; // obtain the patch somewhere here
    /// let current_version = String::new(); // ideally we would read a file here
    /// let new_version = patch.apply(current_version);
    /// ```
    pub fn apply(&self, source: String) -> String {
        let mut lines: Vec<&str> = source.split("\n").collect();

        for patch in &self.1 {
            match patch.1 {
                ChangeMode::Added => {
                    if patch.0 > lines.len() {
                        lines.push(&patch.2);
                        continue;
                    }

                    // insert
                    lines.insert(patch.0, &patch.2);
                    ()
                }
                ChangeMode::Deleted => {
                    lines.remove(patch.0);
                    ()
                }
            }
        }

        lines.join("\n")
    }
}

/// A list of changes to many files (paths are relative to the working tree)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    /// A list of files and their changes
    /// 0: old file source
    /// 1: changes
    pub files: BTreeMap<String, PatchFile>,
}

impl Patch {
    /// Create a [`Patch`] using the diff of two strings
    pub fn from_file(path: String, old: String, new: String) -> Patch {
        use similar::{ChangeTag, TextDiff};

        // create diff
        let diff = TextDiff::from_lines(&old, &new);

        // create patch
        let mut out = Patch {
            files: BTreeMap::new(),
        };

        out.files
            .insert(path.clone(), PatchFile(old.clone(), Vec::new()));

        let this = out.files.get_mut(&path).unwrap();

        for change in diff.iter_all_changes() {
            let mode = match change.tag() {
                ChangeTag::Insert => ChangeMode::Added,
                ChangeTag::Delete => ChangeMode::Deleted,
                ChangeTag::Equal => continue, // we don't store these, we only care about actual changes
            };

            this.1.push((
                change
                    .old_index()
                    .unwrap_or(change.new_index().unwrap_or(0)),
                mode,
                change.value().to_string(),
            ))
        }

        // return
        out
    }

    /// Render the patch into an array of strings
    pub fn render(&self) -> Vec<String> {
        let mut patches = Vec::new();
        let mut total_changes = 0;
        let mut total_additions = 0;
        let mut total_deletions = 0;

        for file in &self.files {
            let spacing = file
                .1
                 .0
                .split("\n")
                .collect::<Vec<&str>>()
                .len()
                .to_string()
                .len()
                + 8;

            let header = format!(
                // patch header
                "\x1b[1m{}:\n{}\n\x1b[0m\u{0096}",
                file.0,
                "=".repeat(file.0.len() + 1)
            );

            let mut out = String::new();

            let changes_iter = file.1 .1.iter();
            let mut consumed = Vec::new();
            for (i, line) in file.1 .0.split("\n").enumerate() {
                // check if the line was deleted
                if let Some(change) = changes_iter
                    .clone()
                    .find(|c| (c.0 == i) && (c.1 == ChangeMode::Deleted))
                {
                    out.push_str(&format!(
                        "\x1b[94m\u{0098}{}\u{009C} {} @@\x1b[0m \x1b[91m- \u{0002}{line}\u{0003}\n",
                        i + 1,
                        " ".repeat(spacing - i.to_string().len() - 1)
                    ));

                    consumed.push(change);
                    continue; // this line was deleted so we shouldn't render the normal line
                }

                // push normal line
                out.push_str(&format!(
                    "\x1b[94m\u{0098}{}\u{009C} {} @@\x1b[0m \x1b[2m\u{2022} \u{0002}{line}\u{0003}\n",
                    i + 1,
                    " ".repeat(spacing - i.to_string().len() - 1)
                ));
            }

            // add new lines
            let mut lines: Vec<String> = Vec::new();

            for r#ref in out.split("\n") {
                // own split
                lines.push(r#ref.to_owned())
            }

            for change in changes_iter {
                if consumed.contains(&change) {
                    // don't process changes we've already consumed
                    // these should only be deletions
                    continue;
                }

                lines.insert(
                    // we're adding 1 to the position so that it is rendered after the removal
                    change.0 + 1,
                    format!(
                        "\x1b[94m\u{0098}{}\u{009C} {} @@\x1b[0m \x1b[92m+ \u{0002}{}\u{0003}",
                        change.0 + 1,
                        " ".repeat(spacing - (change.0 + 1).to_string().len() - 1),
                        change.2.replace("\n", "")
                    ),
                );
            }

            // create footer
            let summary = file.1.summary();

            let mut footer = "\x1b[0m\u{0097}\n\x1b[1m".to_string();
            footer.push_str(&"=".repeat(file.0.len() + 1));
            footer.push_str(&format!(
                "\x1b[0m\n{} total changes \u{2022} \x1b[92m{} additions\x1b[0m \u{2022} \x1b[91m{} deletions\x1b[0m",
                summary.0, // total
                summary.1, // additions
                summary.2, // deletions
            ));

            total_changes += summary.0;
            total_additions += summary.1;
            total_deletions += summary.2;

            // ...
            patches.push(format!("{header}{}{footer}", lines.join("\n\x1b[0m")))
        }

        patches.push(format!("{} total changes \u{2022} \x1b[92m{} additions\x1b[0m \u{2022} \x1b[91m{} deletions\x1b[0m", total_changes, total_additions, total_deletions));
        patches
    }

    /// Render the patch into an array of HTML strings
    pub fn render_html(&self) -> Vec<String> {
        let replacements = vec![
            // &gt;
            (">", "&gt;"),
            // &lt;
            ("<", "&lt;"),
            // reset (close)
            ("\x1b[0m", "</span>"),
            // bold
            (
                "\x1b[1m",
                "<span style=\"font-weight: bold\" class=\"lily:1m\" role=\"bold\">",
            ),
            // faint
            (
                "\x1b[2m",
                "<span style=\"opacity: 75%\" class=\"lily:2m\" role=\"fade\">",
            ),
            // blue
            (
                "\x1b[94m",
                "<span style=\"color: blue\" class=\"lily:94m\" role=\"extra\">",
            ),
            // green
            (
                "\x1b[92m",
                "<span style=\"color: green\" class=\"lily:92m\" role=\"addition\">",
            ),
            // red
            (
                "\x1b[91m",
                "<span style=\"color: red\" class=\"lily:91m\" role=\"deletion\">",
            ),
            // open <code>
            ("\u{0002}", "<code class=\"lily:u0002\" role=\"line\">"),
            // close <code>
            ("\u{0003}", "</code><!-- lily:u0003 -->"),
            // line number
            (
                "\u{0098}",
                "<code class=\"lily:u0098\" role=\"line-number\">",
            ),
            // close line number
            ("\u{009C}", "</code><!-- lily:u009C -->"),
            // open guarded area
            (
                "\u{0096}",
                "<pre class=\"lily:u0096\" role=\"source-display\" style=\"max-width: 100%; overflow-x: auto\">",
            ),
            // close guarded area
            ("\u{0097}", "</pre><!-- lily:0097 -->"),
        ];

        // render for terminal
        let terminal_render = self.render();

        // edit output
        let mut out = Vec::new();

        for mut output in terminal_render {
            for replacement in &replacements {
                output = output.replace(replacement.0, replacement.1);
            }

            out.push(format!("<pre class=\"lily:patch\">{output}</pre>"));
        }

        // return
        out
    }
}
