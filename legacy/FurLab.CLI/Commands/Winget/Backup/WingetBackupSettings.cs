using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Winget.Backup;

/// <summary>
/// Settings for the winget backup command.
/// </summary>
public sealed class WingetBackupSettings : CommandSettings
{
    /// <summary>
    /// Gets the output directory path.
    /// </summary>
    [CommandOption("-o|--output")]
    [Description("Output directory (default: current directory)")]
    public string? Output { get; init; }
}
