# Contributing

* If you encounter a bug and want to report it, please visit the [*Issues*](https://github.com/chfoo/warcat-rs/issues) page. Try searching if the problem already exists to help avoiding duplicate reports. When reporting bugs, try to fill out as much as the template as possible.
* If there is something limiting the functionality of the software/library and you have details on greatly improving it, file a feature request in *Issues* page as well.
* If you want to contribute some bug fixes, documentation, tests, examples, please feel free to submit a Pull Request. If you want to submit a feature and unsure whether it is useful, feel free to file a Issue first.
* If you need help using Warcat, brainstorming ideas, or want to have a general discussion, please use the [*Discussions*](https://github.com/chfoo/warcat-rs/discussions) page instead. Keeping the Issues page on-topic will help make it organized.

## Style guide

* Please configure your IDE to use [Rustfmt](https://github.com/rust-lang/rustfmt). This is the code style formatting used by the project.
* Also configure your IDE to use [Clippy](https://github.com/rust-lang/rust-clippy). This is optional but recommended.
  * Important: CLI code is put under the `bin` feature which is not on by default. (This is a workaround to keep library crate lightweight.) You need to configure your IDE/Clippy to enable the `bin` feature.
* There is an inadvertent use mixed of line endings (CRLF/LF). For old files, please keep them as is for now. For new files, use LF.