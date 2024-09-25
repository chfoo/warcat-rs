# Installation

## Downloads

Downloads are available on [Releases](https://github.com/chfoo/warcat-rs/releases) page.

### Supported platforms

Windows:

* x86_64: 64-bit CPUs, for most devices
* aarch64: 64-bit ARM CPUs, typically for special laptops

macOS:

* aarch64: For newer devices using M1 or newer chipsets
* x86_64: For older devices using Intel CPUs

Linux:

* x86_64: 64-bit CPUs, for most devices
* aarch64: for devices with 64-bit ARM CPUs

### Manually install (optional)

The program does not need to be installed and can be run directly. However, it may be more convenient and organized to install it. The following instructions apply to your user account and not the system.

### Windows

Place the executable in the  `%LOCALAPPDATA%\Programs\warcat\bin\` folder. To access the Programs folder, press Windows+R and open `%LOCALAPPDATA%\Programs`. Then, create the folders needed if they do not exist.

Ensure it is in `Path` the environment variable. To edit them, press Windows+R and open
`rundll32 sysdm.cpl,EditEnvironmentVariables`. Then,

1. Under User variables, select Path
2. Press "Edit..." to open a dialog window with a list
3. Press "New" to edit a blank line
4. Enter `%LOCALAPPDATA%\Programs\warcat\bin\` in the list.
5. Press "OK" to close the dialog window with a list
6. Press "OK" to save changes
7. If you have any opened Console/Terminal windows, close and reopen them again for changes to take effect.

### macOS or Linux

Place the binary to the `$HOME/.local/bin` directory. You may need to create the directory if it does not exist.

Ensure it is in the `PATH` environment variable. Check if this section is in `$HOME/.profile` configuration file:

```sh
if [ -d "$HOME/.local/bin" ] ; then
    PATH="$HOME/.local/bin:$PATH"
fi
```

If not, add it. Then, log out and in for changes to take effect. (If you do not want to close existing terminals windows, run `source $HOME/.profile`.)

## Compiling it yourself

Set up a [Rust environment](https://www.rust-lang.org/tools/install). The latest version of Rust should work. (Rust versions ≥ 1.80, < 2.0 are supported.)

Once you have Rust installed, use the cargo build tool:

```sh
cargo build --release
```

The program will be placed in the `target` directory. You can run it as is, or install it to PATH as described in the previous section.