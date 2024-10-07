# Installation

## Unzip the compressed file

Before the application can be run, it needs to be unzipped from the compressed file you have downloaded.

* How to [unzip on Windows](https://support.microsoft.com/en-us/windows/zip-and-unzip-files-f6dde0a7-0fec-8294-e1d3-703ed85e7ebc)
* How to [unzip on macOS](https://support.apple.com/en-us/guide/mac-help/mchlp2528/mac)

## Installer

The application supports installing itself which is the default behavior. You can double-click to run it.

### Disable the installer functionality

To run the application as a standalone program, remove the "-installer" suffix from the filename.

To manually install, see [this section](install_manual.md).

## Common problems

### Windows

The application is not a signed application and Windows may refuse to run it. To allow an exception, click on "Details" and select "Run anyway".

### macOS

The application is not a signed application and macOS will refuse to run it by default.

To allow an exception, right-click the program file and select "Open" or [follow these instructions](https://support.apple.com/en-us/guide/mac-help/mh40616/mac).

### macOS and Linux

If you get an error saying it is not an executable program, you need to set the executable bit of the file. To do this, open the terminal and run a similar command to `chmod +x warcat-1.2.3`.