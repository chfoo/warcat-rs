# Introduction to the CLI application

To begin, open the terminal application.

On Windows, right-click the Start icon or press Windows+X. Then, select [Terminal](https://learn.microsoft.com/en-us/windows/terminal/).

On macOS, open Finder, then select Applications, Utilities, then [Terminal](https://support.apple.com/en-us/guide/terminal/apd5265185d-f365-44cb-8b09-71a064a42125/mac).

On Linux, open Applications. Select System, then Terminal. Or, search for "terminal".

The terminal application will then present a command line interface (CLI). On Windows, this is [PowerShell](https://learn.microsoft.com/en-us/powershell/). On macOS or Linux, this is typically [Bash shell](https://www.gnu.org/software/bash/manual/bash.html).

If you have the application is under the PATH environment variable, to run it, type:

```sh
warcat
```

and press enter.

Or, enter the location of the executable directly. For example (Windows):

```powershell
.\Downloads\warcat.exe
```

macOS/Linux:
```sh
./Downloads/warcat
```

and press enter.

If it is successful, the warcat application will display help information.

Entering

```sh
warcat help
```

will also show a list of commands and options. `help` is known as an argument that is passed to the program.

For example using the `list` command:

```sh
warcat list --input my_warc_file.warc.gz
```

The above command has 3 arguments to the program:

1. `list` is the command.
2. `--input` is an option. It starts with 2 hyphens. This specifies that the program should accept an input filename.
3. `my_warc_file.warc.gz` is a value to the `input` option.

If you need help in a command, enter something like:

```sh
warcat help list
```