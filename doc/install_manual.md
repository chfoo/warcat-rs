# Manual installation (advanced)

If you do not want to use the automated installer, you can follow instructions below to install it to your user account.

## Windows

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

## macOS or Linux

Place the binary to the `$HOME/.local/bin` directory. You may need to create the directory if it does not exist.

Ensure it is in the `PATH` environment variable. Check if this section is in `$HOME/.profile` configuration file:

```sh
if [ -d "$HOME/.local/bin" ] ; then
    PATH="$HOME/.local/bin:$PATH"
fi
```

If not, add it. Then, log out and in for changes to take effect. (If you do not want to close existing terminal windows, run `source $HOME/.profile`.)