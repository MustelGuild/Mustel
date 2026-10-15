using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Winget.Restore;

/// <summary>
/// Settings for the winget restore command.
/// </summary>
public sealed class WingetRestoreSettings : CommandSettings
{
    /// <summary>
    /// Gets the input file path.
    /// </summary>
    [CommandOption("-i|--input")]
    [Description("Input file (default: backup-winget.json in current directory)")]
    public string? Input { get; init; }
}
